use std::borrow::Cow;
use std::env;
use std::net::{Ipv4Addr, SocketAddr};
use std::{collections::HashMap, io::Cursor};
use tokio::net::TcpListener;
mod utils;
use anyhow::bail;
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
use chrono::{DateTime, NaiveDateTime, Utc};
use csv::Error;
use dms_viewer::Variant;
use maud::{html, Markup, PreEscaped, DOCTYPE};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, query_scalar, PgPool};
use tower_http::services::ServeDir;
use tracing::{debug, info, warn};
use utils::{Normalizer, PosColor};
#[derive(Clone, Debug)]
pub struct EnvironmentVariables {
    pub database_url: Cow<'static, str>,
    pub port: u16,
}

impl EnvironmentVariables {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv()?;
        Ok(Self {
            database_url: match env::var("DATABASE_URL") {
                Ok(url) => url.into(),
                Err(err) => bail!("missing DATABASE_URL: {err}"),
            },
            port: match env::var("PORT") {
                Ok(port) => port.parse()?,
                _ => 8000,
            },
        })
    }
}

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    env: EnvironmentVariables,
}
impl AppState {
    pub async fn from_env() -> anyhow::Result<Self> {
        let env = EnvironmentVariables::from_env()?;
        Ok(Self {
            pool: PgPool::connect(&env.database_url).await?,
            env: EnvironmentVariables::from_env()?,
        })
    }
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
                meta name="htmx-config" content="{\"responseHandling\": [{\"code\":\".*\", \"swap\": true}]}"{}

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
                hx-target="this"
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
                    tbody id="dms-table-body"{}
                }
            }
            script src="assets/viewer.js"{}
            div id="variant-view"{
                div id="variant-view-table" x-data x-init="initMolstar()"{
                    div id="variant-view-header"{
                        div {"Protein: "}
                        div {"Condition: "}
                        div {"Position: "}
                        div {"Amino Acid: "}
                        div {"log2 Fold Change: "}
                        div {"log2 Std Error: "}
                        div {"z-statistic: "}
                        div {"p-value: "}
                    }
                    div id="variant-view-body"{}
                }
            }


            div id="structure"{}




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
            FROM variant
            JOIN protein ON variant.protein_id = protein.id
            WHERE protein.name = $1;
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
    let rows = sqlx::query!("SELECT DISTINCT id, name FROM protein;")
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
                            option value=(protein.name) { (protein.name) }
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
                        variant.id,
                        variant.chunk,
                        variant.pos,
                        variant.p_value,
                        variant.created_on,
                        variant.log2_fold_change,
                        variant.log2_std_error,
                        variant.statistic,
                        variant.condition,
                        variant.aa,
                        variant.version,
                        protein.name as protein
                    FROM variant
                    JOIN protein ON variant.protein_id = protein.id
                    WHERE protein.name = $1
                    AND variant.condition = $2
                    ORDER BY variant.pos, variant.aa
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
                        variant.id,
                        variant.chunk,
                        variant.pos,
                        variant.p_value,
                        variant.created_on,
                        variant.log2_fold_change,
                        variant.log2_std_error,
                        variant.statistic,
                        variant.condition,
                        variant.aa,
                        variant.version,
                        protein.name as protein,
                        ROW_NUMBER() OVER (
                            PARTITION BY variant.pos
                            ORDER BY
                                CASE $5
                                    WHEN 'MostSignificantPValue' THEN variant.p_value
                                    WHEN 'LargestLog2FoldChange' THEN -variant.log2_fold_change
                                    WHEN 'LargestZStatistic' THEN -variant.statistic
                                    ELSE NULL
                                END ASC
                        ) AS rn
                    FROM variant
                    JOIN protein ON variant.protein_id = protein.id
                    WHERE protein.name = $1
                    AND variant.condition = $2
                )
                SELECT
                    id,
                    chunk,
                    pos,
                    p_value,
                    created_on,
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
    if variants.is_empty() {
        return (StatusCode::NOT_FOUND, html!("No variants found")).into_response();
    }
    if let Some(max_pos) = &variants.iter().map(|variant| variant.pos).max() {
        if let Some(min_pos) = &variants.iter().map(|variant| variant.pos).min() {
            let query_length: i32 = (variants.len() / &AMINO_ACIDS.len()) as i32;
            info!("{}", query_length);
            let positions: Vec<i32> = (*min_pos..=*max_pos).collect();
            debug!("{:?}", &positions);

            match sqlx::query_scalar!(
                r#"
                select
                    max(abs(
                        case $3
                            when 'p_value' then variant.p_value
                            when 'log2_fold_change' then variant.log2_fold_change
                            when 'statistic' then variant.statistic
                        end
                    ))
                from variant
                join protein on variant.protein_id = protein.id
                where protein.name = $1 and variant.condition = $2;"#,
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
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                html!("couldn't find min pos"),
            )
                .into_response();
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
    html! { div id="upload-file-message" {(message)}
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
                let (variants, errors) = read_tsv(file_data, &protein);

                match insert(&state.pool, &variants, &protein).await {
                    Ok(result) => {
                        let mut res = upload_file_component_with_message(&format!(
                            "File successfully uploaded. {result} rows affected with {} errors",
                            errors.len()
                        ))
                        .into_response();
                        res.headers_mut()
                            .insert("HX-Trigger", HeaderValue::from_static("load-condition"));
                        res
                    }
                    Err(err) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        upload_file_component_with_message(&err.to_string()),
                    )
                        .into_response(),
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
fn read_tsv(file_contents: Bytes, protein: &str) -> (Vec<Variant>, Vec<Error>) {
    info!("reading file");
    let cursor = Cursor::new(file_contents);
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t') // Specify TSV format
        .has_headers(true)
        .from_reader(cursor);
    let created_on = Utc::now().naive_utc();
    let mut variants = vec![];
    let mut errors = vec![];
    for result in reader.deserialize::<Variant>() {
        match result {
            Ok(mut variant) => {
                variant.protein = protein.to_string();
                variant.created_on = created_on;
                variants.push(variant);
            }
            Err(e) => {
                errors.push(e);
            }
        }
    }
    (variants, errors)
}

async fn get_variant_by_id(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Query(_params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    // info!("Acquired variant {id}");
    let pool = &state.pool;
    if let Ok(variant) = query_as!(
        Variant,
        r#"
        SELECT
                variant.id,
                variant.chunk,
                variant.pos,
                variant.p_value,
                variant.created_on,
                variant.log2_fold_change,
                variant.log2_std_error,
                variant.statistic,
                variant.condition,
                variant.aa,
                variant.version,
                protein.name as protein
            FROM variant
            JOIN protein ON variant.protein_id = protein.id
            WHERE variant.id = $1;
    "#,
        id
    )
    .fetch_one(pool)
    .await
    {
        let mut res = html!(
            div{(variant.protein)}
            div{(variant.condition)}
            div{(variant.pos)}
            div{(variant.aa)}
            div{(format!("{:.3}",variant.log2_fold_change))}
            div{(format!("{:.3}",variant.log2_std_error))}
            div{(format!("{:.3}",variant.statistic))}
            div{(format!("{:.5}",variant.p_value))}

            script {(PreEscaped(format!("focusVariant({})",variant.pos)))}
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
#[derive(sqlx::FromRow)]
struct Id {
    id: i32,
}
async fn insert(pool: &PgPool, variants: &[Variant], protein: &str) -> Result<u64, sqlx::Error> {
    info!("Inserting {} variants into db", variants.len());
    let mut txn = pool.begin().await?;
    let chunks: Vec<i32> = variants.iter().map(|v| v.chunk).collect();
    let positions: Vec<i32> = variants.iter().map(|v| v.pos).collect();
    let conditions: Vec<String> = variants.iter().map(|v| v.condition.clone()).collect();
    let aas: Vec<String> = variants.iter().map(|v| v.aa.clone()).collect();
    let log2_fold_changes: Vec<f64> = variants.iter().map(|v| v.log2_fold_change).collect();
    let log2_std_errors: Vec<f64> = variants.iter().map(|v| v.log2_std_error).collect();
    let statistics: Vec<f64> = variants.iter().map(|v| v.statistic).collect();
    let p_values: Vec<f64> = variants.iter().map(|v| v.p_value).collect();
    let versions: Vec<String> = variants.iter().map(|v| v.version.clone()).collect();
    let created_ons: Vec<NaiveDateTime> = variants.iter().map(|v| v.created_on).collect();
    let protein_id: i32 = query_scalar!("SELECT id FROM protein WHERE name = $1", protein)
        .fetch_one(&mut *txn) // Fetch one record, assuming the protein exists
        .await?;
    info!("Found protein {} at id {}", protein, protein_id);
    let sql = r#"
            INSERT INTO variant
            (
                chunk,
                pos,
                condition,
                aa,
                log2_fold_change,
                log2_std_error,
                statistic,
                p_value,
                version,
                protein_id,
                created_on
            )
            SELECT * FROM UNNEST(
                $1::INT8[],
                $2::INT8[],
                $3::VARCHAR(30)[],
                $4::VARCHAR(30)[],
                $5::DOUBLE PRECISION[],
                $6::DOUBLE PRECISION[],
                $7::DOUBLE PRECISION[],
                $8::DOUBLE PRECISION[],
                $9::VARCHAR(30)[],
                $10::INT8[],
                $11::TIMESTAMP[]
            ) RETURNING id;
        "#;
    let rows: Vec<Id> = query_as::<_, Id>(sql)
        .bind(chunks)
        .bind(positions)
        .bind(conditions)
        .bind(aas)
        .bind(log2_fold_changes)
        .bind(log2_std_errors)
        .bind(statistics)
        .bind(p_values)
        .bind(versions)
        .bind(vec![protein_id; variants.len()]) // A vector filled with the protein_id for all rows
        .bind(created_ons)
        .fetch_all(pool)
        .await?;

    txn.commit().await?;
    let rows_affected = rows.into_iter().map(|r| r.id).len() as u64;
    info!("rows affected {}", rows_affected);
    Ok(rows_affected)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Welcome to DeepScan!");
    let state = AppState::from_env().await?;
    let listener =
        TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, state.env.port))).await?;

    let app = Router::new()
        .route("/", get(main_content))
        .route("/variants", get(get_variants))
        .route("/proteins", get(get_proteins))
        .route("/conditions", get(get_conditions))
        .route("/upload", post(upload_file))
        .route("/variants/:id", get(get_variant_by_id))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 100000))
        .with_state(state)
        .nest_service("/assets", ServeDir::new("assets"));
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
    Ok(())
}
