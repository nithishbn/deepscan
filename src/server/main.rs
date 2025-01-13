use std::{collections::HashMap, io::Cursor};
mod utils;
use axum::extract::Path;
use axum::{
    body::Bytes,
    debug_handler,
    extract::{DefaultBodyLimit, Multipart, Query, State},
    http::{HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use chrono::DateTime;
use csv::Error;
use dms_viewer::Variant;
use maud::{html, Markup, PreEscaped, DOCTYPE};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, query, query_as, query_scalar, PgPool};
use tower_http::services::ServeDir;
use tracing::{debug, error, info, warn};
use utils::{Normalizer, PosColor};

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}
#[derive(Deserialize, Debug)]
enum PositionFilter {
    MostSignificantPValue,
    LargestLog2FoldChange,
    LargestZStatistic,
    NoOrder,
}

#[derive(Serialize, Deserialize, Debug)]
enum Paint {
    #[serde(alias = "p_value")]
    PValue,
    #[serde(alias = "log2_fold_change")]
    Log2FoldChange,
    #[serde(alias = "statistic")]
    ZStatistic,
}
impl std::fmt::Display for Paint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let output = match self {
            Paint::Log2FoldChange => "log2_fold_change",
            Paint::PValue => "p_value",
            Paint::ZStatistic => "statistic",
        };
        write!(f, "{}", output)
    }
}
// impl ToString for Paint {
// fn to_string(&self) -> String {
// match self {
// Paint::PValue => "p_value".to_string(),
// Paint::Log2FoldChange => "log2_fold_change".to_string(),
// Paint::ZStatistic => "statistic".to_string(),
// }
// }
// }
//
// impl ToString for PositionFilter {
// fn to_string(&self) -> String {
// match self {
// PositionFilter::MostSignificantPValue => "MostSignificantPValue".to_string(),
// PositionFilter::LargestLog2FoldChange => "LargestLog2FoldChange".to_string(),
// PositionFilter::LargestZStatistic => "LargestZStatistic".to_string(),
// PositionFilter::NoOrder => "NoOrder".to_string(),
// }
// }
// }
impl std::fmt::Display for PositionFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let output = match self {
            PositionFilter::MostSignificantPValue => "MostSignificantPValue",
            PositionFilter::LargestLog2FoldChange => "LargestLog2FoldChange",
            PositionFilter::LargestZStatistic => "LargestZStatistic",
            PositionFilter::NoOrder => "NoOrder",
        };
        write!(f, "{}", output)
    }
}

#[derive(Deserialize)]
struct TableParams {
    protein: String,
    condition: String,
    position_filter: PositionFilter,
    paint: Paint,
    page: i32,
}

const AMINO_ACIDS: [&str; 21] = [
    "*", "A", "C", "D", "E", "F", "G", "H", "I", "K", "L", "M", "N", "P", "Q", "R", "S", "T", "V",
    "W", "Y",
];
const PAGE_SIZE: i32 = 4200;
fn base(content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=3, maximum-scale=1, user-scalable=no" {}
                title { "DeepScan" }

                // Styles
                link rel="stylesheet" href="assets/style.css"{}
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/pdbe-molstar@3.3.0/build/pdbe-molstar-light.css"{}

                // Htmx + Alpine
                script src="assets/htmx.min.js" {}
                script src="https://cdn.jsdelivr.net/npm/pdbe-molstar@3.3.0/build/pdbe-molstar-plugin.js"{}
                script src="//unpkg.com/alpinejs" defer {}

            }
            body{

                (content)
            }
        }
    }
}
#[debug_handler]
async fn main_content() -> Markup {
    base(html! {
        h1 id = "page-title" { span id="page-title-start"{"// "} span id="page-title-end"{"DEEPSCAN"} }
        div id="select-and-upload"{
            form
                hx-post="/upload"
                hx-encoding="multipart/form-data"
                hx-include="[name='protein']"
                hx-indicator="#upload-indicator"
                {
                p id="upload-indicator" class="htmx-indicator" {"Uploading file..."}
                input type="file" name="file" {}
                button{ "Upload" }
            }
            form id="selection-form"
                hx-get="/proteins"
                hx-trigger="load delay:0.5s"
                {
                }
        }

        div id="full-view"{
            div id="dms-table-container"{
                table id="dms-table"{
                    thead{
                        tr{
                            th{" "}
                            @for amino_acid in &AMINO_ACIDS{
                                th { (amino_acid)}
                            }
                        }
                    }
                    tbody
                    id="dms-table-body"
                    {}
                }
            }
            script src="assets/viewer.js"{}
                    div id="full-variant-view" x-data x-init="initMolstar()" {
                    div id="variant-view"{
                        div id="variant-view-body"{}
                        div id="structure"{
                        }
                    }
            }




        }

    })
}

async fn get_conditions(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    info!("getting conditions");
    let protein = params.get("protein").expect("protein not found");
    let rows = sqlx::query!(
        r#"
            SELECT DISTINCT condition
            FROM dms
            JOIN proteins ON dms.protein_id = proteins.id
            WHERE proteins.protein = $1;
            "#,
        protein
    )
    .fetch_all(&state.pool)
    .await;
    let pdb_id = match protein.as_str() {
        "GLP1R" => "7ki0",
        "GIPR" => "8wa3",
        "RHO" => "1f88",
        _ => "7s15",
    };
    match rows {
        Ok(conditions) => (
            StatusCode::OK,
            [("HX-Trigger", "load-condition")],
            html! {
                @for condition in &conditions{
                    option value=(condition.condition) { (condition.condition) }
                }
                script {
                    (PreEscaped(format!("refresh_and_load_pdb_into_viewer('{}')",pdb_id)))
                }
            },
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [("", "")],
            html! {
                p { "Error fetching conditions: " (err) }
            },
        ),
    }
}

async fn get_proteins(State(state): State<AppState>) -> Markup {
    info!("getting proteins");
    let rows = sqlx::query!("SELECT DISTINCT id, protein FROM proteins;")
        .fetch_all(&state.pool)
        .await;
    match rows {
        Ok(proteins) => {
            html! {
            #protein-select-div .select-div{
                label for="protein" id="protein-select-label"{"Protein"}
                select id="protein-select" name="protein"
                    hx-get="/conditions"
                    hx-include="this"
                    hx-target="#condition-select"
                    hx-trigger="change,load delay:0.1s"
                    {
                        @for protein in &proteins{
                            option value=(protein.protein) { (protein.protein) }
                        }
                    }
            }

            #condition-select-div .select-div{
            label for="condition" id="condition-select-label"{"Condition"}
            select id="condition-select" name="condition"
                hx-get="/variants?page=1"
                hx-indicator="#loading"
                hx-include="[name='protein'],[name='condition'],[name='position_filter'],[name='paint']"
                hx-target="#dms-table-body"
                hx-trigger="change,load-condition from:body delay:0.5s "
                {}
            }

            #position-filter-select-div .select-div{
            label id="label-position-filter-select" for="position_filter"{"Select"}
            select id="position-filter-select" name="position_filter"
                hx-get="/variants?page=1"
                hx-indicator="#loading"
                hx-include="[name='protein'],[name='condition'],[name='paint']"
                hx-target="#dms-table-body"
                hx-trigger="change,load-condition from:body delay:0.5s "
            {
                option value=("NoOrder") { ("No Order") }
                option value=("MostSignificantPValue") { ("Most significant p value") }
                option value=("LargestLog2FoldChange") { ("Largest log2 Fold Change") }
                option value=("LargestZStatistic") { ("Largest z statistic") }
            }
            }

            #paint-by-filter-select-div .select-div{
            label id="label-paint-by-select" for="paint"{"Paint By"}
            select id="paint-by-select" name="paint"
                hx-get="/variants?page=1"
                hx-indicator="#loading"
                hx-include="[name='protein'],[name='condition'],[name='position_filter']"
                hx-target="#dms-table-body"
                hx-trigger="change,load-condition from:body delay:0.5s "
            {
                option value=("Log2FoldChange") { ("log2 Fold Change") }
                option value=("PValue") { ("p value") }
                option value=("ZStatistic") { ("z statistic") }
            }}


            div id="loading" class="htmx-indicator"{"Loading..."}

            }
        }

        Err(err) => {
            html! {
                p { "Error fetching proteins: " (err) }
            }
        }
    }
}

async fn get_variants(
    State(state): State<AppState>,
    Query(params): Query<TableParams>,
) -> impl IntoResponse {
    let TableParams {
        ref protein,
        ref condition,
        page,
        ref position_filter,
        ref paint,
    } = params;
    info!(
        "Getting variant for protein = {}, condition = {} and page = {} and order={:?}",
        protein, condition, page, position_filter
    );
    let offset = (page - 1) * PAGE_SIZE;
    info!("offset: {offset}");
    let variants: Vec<Variant> = match position_filter {
        PositionFilter::NoOrder => sqlx::query_as!(
            Variant,
            r#"SELECT
                        dms.id,
                        dms.chunk,
                        dms.pos,
                        dms.p_value,
                        dms.created_at,
                        dms.log2_fold_change,
                        dms.log2_std_error,
                        dms.statistic,
                        dms.condition,
                        dms.aa,
                        dms.version,
                        proteins.protein
                    FROM dms
                    JOIN proteins ON dms.protein_id = proteins.id
                    WHERE proteins.protein = $1
                    AND dms.condition = $2
                    ORDER BY dms.pos, dms.aa
                    LIMIT $3 OFFSET $4
                    "#,
            protein,
            condition,
            PAGE_SIZE as i64,
            offset as i64
        )
        .fetch_all(&state.pool)
        .await
        .unwrap(),
        _ => sqlx::query_as!(
            Variant,
            r#"
                WITH ranked_variants AS (
                    SELECT
                        dms.id,
                        dms.chunk,
                        dms.pos,
                        dms.p_value,
                        dms.created_at,
                        dms.log2_fold_change,
                        dms.log2_std_error,
                        dms.statistic,
                        dms.condition,
                        dms.aa,
                        dms.version,
                        proteins.protein,
                        ROW_NUMBER() OVER (
                            PARTITION BY dms.pos
                            ORDER BY
                                CASE $5
                                    WHEN 'MostSignificantPValue' THEN dms.p_value
                                    WHEN 'LargestLog2FoldChange' THEN -dms.log2_fold_change
                                    WHEN 'LargestZStatistic' THEN -dms.statistic
                                    ELSE NULL
                                END ASC
                        ) AS rn
                    FROM dms
                    JOIN proteins ON dms.protein_id = proteins.id
                    WHERE proteins.protein = $1
                    AND dms.condition = $2
                )
                SELECT
                    id,
                    chunk,
                    pos,
                    p_value,
                    created_at,
                    log2_fold_change,
                    log2_std_error,
                    statistic,
                    condition,
                    aa,
                    version,
                    protein
                FROM ranked_variants
                WHERE rn = 1
                LIMIT $3 OFFSET $4
                "#,
            protein,
            condition,
            PAGE_SIZE as i64,
            offset as i64,
            position_filter.to_string()
        )
        .fetch_all(&state.pool)
        .await
        .unwrap(),
    };

    if let Some(max_pos) = &variants.iter().map(|variant| variant.pos).max() {
        if let Some(min_pos) = &variants.iter().map(|variant| variant.pos).min() {
            let query_length: i32 = (variants.len() / &AMINO_ACIDS.len()) as i32;
            info!("{}", query_length);
            let positions: Vec<i32> = (*min_pos..=*max_pos).collect();
            debug!("{:?}", &positions);
            let query = format!(
                r#"
                select
                    max(abs({}))
                from dms
                join proteins on dms.protein_id = proteins.id
                where proteins.protein = $1 and dms.condition = $2;"#,
                paint.to_string()
            );
            match sqlx::query_scalar!(
                r#"
                select
                    max(abs(
                        case $3
                            when 'p_value' then dms.p_value
                            when 'log2_fold_change' then dms.log2_fold_change
                            when 'statistic' then dms.statistic
                        end
                    ))
                from dms
                join proteins on dms.protein_id = proteins.id
                where proteins.protein = $1 and dms.condition = $2;"#,
                protein,
                condition,
                paint.to_string()
            )
            .fetch_one(&state.pool)
            .await
            {
                Ok(max_abs_option) => match max_abs_option {
                    Some(max_abs) => {
                        let normalizer = utils::Normalizer { max_abs };
                        let pos_color_pairs: Vec<PosColor> = variants
                            .iter()
                            .map(|variant| PosColor {
                                pos: variant.pos,
                                color: normalizer.get_color_hex(variant.log2_fold_change),
                            })
                            .collect();
                        let mut res = (
                            StatusCode::OK,

                            html!(
                                @for pos in &positions{
                                    tr{
                                        th scope="row"{(pos)}
                                        @for amino_acid in &AMINO_ACIDS{
                                            @let end_of_row = (query_length!=0 && pos == &(max_pos - 15)) && (&amino_acid == &AMINO_ACIDS.last().unwrap());
                                            (get_variant_cell(&variants, amino_acid, pos, &params, end_of_row,&normalizer))
                                        }
                                    }
                                }
                                script {(PreEscaped(format!("colorVariants({})",serde_json::json!(pos_color_pairs))))}


                            )
                            ).into_response();
                        res.headers_mut()
                            .insert("Cache-Control", HeaderValue::from_static("max-age=100"));
                        return res;
                    }
                    None => {
                        warn!("error");

                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            html!(div{"max_abs not found"}),
                        )
                            .into_response()
                    }
                },
                Err(err) => {
                    warn!("error");
                    (StatusCode::INTERNAL_SERVER_ERROR, html!(div{(err)})).into_response()
                }
            }
        } else {
            return (StatusCode::INTERNAL_SERVER_ERROR, html!()).into_response();
        }
    } else {
        return (StatusCode::INTERNAL_SERVER_ERROR, html!()).into_response();
    }
}

fn get_variant_cell(
    variants: &[Variant],
    amino_acid: &str,
    pos: &i32,
    params: &TableParams,
    end_of_row: bool,
    normalizer: &Normalizer,
) -> Markup {
    // info!("{}",pos%PAGE_SIZE==0);
    if end_of_row {
        info!("reached end of row")
    };
    for variant in variants {
        if amino_acid == &variant.aa && pos == &variant.pos {
            let color = normalizer.get_color_hex(variant.log2_fold_change);
            if end_of_row {
                info!("emitting end of row td");
                return html!((format_variant_cell(
                    Some(variant.log2_fold_change),
                    pos,
                    amino_acid,
                    variant.id,
                    Some(color.clone())
                ))(format_invisible_lazy_load_cell(&params)));
            } else {
                return format_variant_cell(
                    Some(variant.log2_fold_change),
                    pos,
                    amino_acid,
                    variant.id,
                    Some(color),
                );
            }
        }
    }
    if end_of_row {
        info!("emitting end of row td after not finding ");

        return html!((format_variant_cell(None, pos, amino_acid, None, None))(
            format_invisible_lazy_load_cell(&params)
        ));
    }
    return html!((format_variant_cell(None, pos, amino_acid, None, None)));
}

fn format_variant_cell(
    log2_fold_change: Option<f64>,
    pos: &i32,
    amino_acid: &str,
    variant_id: Option<i32>,
    color: Option<String>,
) -> Markup {
    if let Some(log2_fc) = log2_fold_change {
        if let Some(color) = color {
            if let Some(variant_id) = variant_id {
                return html!(
                    td
                    style=(format!("background-color: {}",color))
                    id=(format!("{pos}{amino_acid}"))
                    class="dms-cell"
                    title=(format!("log2FC: {:.3}, {}{}",log2_fc,pos,amino_acid))
                    hx-get=(format!("variants/{}",variant_id))
                    hx-vals=(format!("{{\"color\":\"{}\"}}",color))
                    hx-trigger="mouseover"
                    hx-target="#variant-view-body"
                {}
                );
            }
        }
    }
    return html!(
        td
        id=(format!("{pos}{amino_acid}"))
        class="dms-cell-no-data"
        title=(format!("log2FC: {:.3}, {}{}","N/A",pos,amino_acid)){});
}

fn format_invisible_lazy_load_cell(params: &TableParams) -> Markup {
    return html!(
        td
        id="invisible-lazy-load-cell"
        hx-trigger="intersect once"
        hx-target="#dms-table-body"
        hx-get=(format!("/variants?page={}&protein={}&condition={}&position_filter={}&paint={}",params.page+1,params.protein,params.condition,params.position_filter,params.paint))
        hx-swap="beforeend"
        {});
}

fn upload_file_component_with_message(message: &str) -> Markup {
    info!("IN");
    html! { p id="upload-file-message" {(message)}
        input type="file" name="file" {}
        button{ "Upload" }
    }
}

#[debug_handler]
async fn upload_file(State(state): State<AppState>, mut multipart: Multipart) -> impl IntoResponse {
    info!("Uploading file");
    let mut protein: Option<String> = None;
    let mut file: Option<Bytes> = None;
    while let Some(field) = multipart.next_field().await.unwrap() {
        if let Some(field_name) = field.name() {
            if field_name == "protein" {
                info!("{:?}", field);
                protein = Some(field.text().await.unwrap());
            } else if field_name == "file" {
                file = Some(field.bytes().await.unwrap().clone().clone());
            }
        }
    }
    match protein {
        Some(protein) => {
            // Proceed if file exists, otherwise return an error
            if let Some(file_data) = file {
                match read_tsv(file_data, &protein) {
                    Ok(variants) => {
                        match insert(&state.pool, &variants, &protein).await {
                            Ok(result) => {
                                let mut res = upload_file_component_with_message(&format!(
                                    "File successfully uploaded. {result} rows affected"
                                ))
                                .into_response();
                                res.headers_mut().insert(
                                    "HX-Trigger",
                                    HeaderValue::from_static("load-condition"),
                                );
                                res
                            }
                            Err(err) => (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                upload_file_component_with_message(&err.to_string()),
                            )
                                .into_response(),
                        }
                        // Return the processed result
                    }
                    Err(err) => {
                        // Handle any error from read_tsv
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            upload_file_component_with_message(&format!(
                                "Error processing file - {err}"
                            )),
                        )
                            .into_response()
                    }
                }
            } else {
                // No file uploaded
                (
                    StatusCode::BAD_REQUEST,
                    upload_file_component_with_message("No file uploaded"),
                )
                    .into_response()
            }
        }
        None => {
            // No protein found
            (
                StatusCode::BAD_REQUEST,
                upload_file_component_with_message("No protein found"),
            )
                .into_response()
        }
    }
}
fn read_tsv(file_contents: Bytes, protein: &str) -> Result<Vec<Variant>, Error> {
    info!("reading file");
    let cursor = Cursor::new(file_contents);
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t') // Specify TSV format
        .has_headers(true)
        .from_reader(cursor);
    let created_at =
        DateTime::from_timestamp(chrono::Utc::now().timestamp(), 0).expect("timestamp failed");
    let mut variants = vec![];
    for result in reader.deserialize::<Variant>() {
        // If deserialization fails, return the error
        let mut variant = result.map_err(|e| {
            error!("Failed to deserialize row: {}", e);
            e
        })?;

        // Modify the variant fields
        variant.protein = protein.to_string();
        variant.created_at = created_at;
        variants.push(variant);
    }
    Ok(variants)
}

async fn get_variant_by_id(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    // info!("Acquired variant {id}");
    let pool = &state.pool;
    let color = params.get("color").expect("color not found");
    if let Ok(variant) = query_as!(
        Variant,
        r#"
        SELECT
                dms.id,
                dms.chunk,
                dms.pos,
                dms.p_value,
                dms.created_at,
                dms.log2_fold_change,
                dms.log2_std_error,
                dms.statistic,
                dms.condition,
                dms.aa,
                dms.version,
                proteins.protein
            FROM dms
            JOIN proteins ON dms.protein_id = proteins.id
            WHERE dms.id = $1;
    "#,
        id
    )
    .fetch_one(pool)
    .await
    {
        let mut res = html!(
                    table{
                        tr{
                            th{"Protein"}
                            td{(variant.protein)}
                        }
                        tr{ th{"Condition"}
                            td{(variant.condition)}
                        }
                        tr{ th{"Position"}
                            td{(variant.pos)}
                        }
                        tr{ th{"Amino Acid"}
                            td{(variant.aa)}
                        }
                        tr{ th{"log2 Fold Change"}
                            td{(format!("{:.3}",variant.log2_fold_change))}
                        }
                        tr{                    th{"log2 Standard Error"}
                            td{(format!("{:.3}",variant.log2_std_error))}

        }
                        tr{                    th{"z-statistic"}
                            td{(format!("{:.3}",variant.statistic))}
                        }
                            tr{th{"p value"}

                                td{(format!("{:.3}",variant.p_value))}
        }
                        }



                )
        .into_response();
        res.headers_mut()
            .insert("Cache-Control", HeaderValue::from_static("max-age=100"));
        return res;
    } else {
        warn!("Variant not found!");
        return html!("Variant not found!").into_response();
    }
}
async fn insert(pool: &PgPool, variants: &[Variant], protein: &str) -> Result<u64, sqlx::Error> {
    info!("Inserting {} variants into db", variants.len());
    let mut txn = pool.begin().await?;
    let protein_id: i32 = query_scalar!("SELECT id FROM proteins WHERE protein = $1", protein)
        .fetch_one(&mut *txn) // Fetch one record, assuming the protein exists
        .await?;
    let mut rows_affected = 0;
    for variant in variants {
        // Perform the insert query
        match query!(
                    "INSERT INTO dms (chunk, pos, condition, aa, log2_fold_change, log2_std_error, statistic, p_value, version, protein_id, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                    &variant.chunk,
                    variant.pos,
                    variant.condition,
                    &variant.aa,
                    variant.log2_fold_change,
                    variant.log2_std_error,
                    variant.statistic,
                    variant.p_value,
                    variant.version,
                    protein_id,
                    variant.created_at
                )
                .execute(&mut *txn)
                .await
                {
                    Ok(result) => {rows_affected+=result.rows_affected();},
                    Err(_)=>{info!("aaaah"); 0;}
                };
    }

    txn.commit().await?;
    info!("rows affected {}", rows_affected);
    Ok(rows_affected)
}

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    tracing_subscriber::fmt::init();
    info!("hello");
    // build our application with a single route
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://postgres@localhost/postgres")
        .await?;
    let state = AppState { pool };
    // run it with hyper on localhost:3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    let app = Router::new()
        .route("/", get(main_content))
        .route("/variants", get(get_variants))
        .route("/proteins", get(get_proteins))
        .route("/conditions", get(get_conditions))
        .route("/upload", post(upload_file))
        .route("/variants/:id", get(get_variant_by_id))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 100000000))
        .with_state(state)
        .nest_service("/assets", ServeDir::new("assets"));
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
    Ok(())
}
