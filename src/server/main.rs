use axum::http::header;
use axum::middleware;
use rand::Rng;
use serde::Deserialize;
use std::net::{Ipv4Addr, SocketAddr};
use std::{collections::HashMap, io::Cursor};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
pub mod utils;
use axum::extract::Path;
use axum::{
    body::Bytes,
    debug_handler,
    extract::{DefaultBodyLimit, Multipart, Query, State},
    http::{HeaderValue, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use chrono::{NaiveDateTime, Utc};
use csv::Error;
use dms_viewer::{
    AppState, Normalizer, Paint, PosColor, PositionFilter, TableParams, Variant, VariantColor,
    AMINO_ACIDS, PAGE_SIZE,
};
use maud::{html, Markup, PreEscaped, DOCTYPE};
use sqlx::{query_as, query_scalar, PgPool};
use tower_http::services::ServeDir;
use tracing::{debug, info, warn};
use utils::set_static_cache_control;

fn base(content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=3, maximum-scale=1, user-scalable=no" {}
                title{ "DMS Viewer" }

                // Styles
                link rel="stylesheet" href="assets/style.css"{}
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/pdbe-molstar@3.3.0/build/pdbe-molstar-light.css"{}

                // Htmx + PDBe Molstar + Alpine
                script src="assets/htmx.min.js" {}
                script src="https://cdn.jsdelivr.net/npm/pdbe-molstar@3.3.0/build/pdbe-molstar-plugin.js"{}
                script src="//unpkg.com/alpinejs" defer {}
                script src="https://cdn.jsdelivr.net/npm/d3@7"{}
                script src="assets/scatter.js"{}

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
    let var_name = html! {
        h1 id = "page-title" {
            span id="page-title-start"{"// "}
            span hx-get="/title?previous=DeepScan" hx-trigger="every 7s" hx-swap="outerHTML swap:1s settle:1s" id="page-title-end" {"DEEPSCAN"}
        }
        div id="select-and-upload"{
            fieldset{
                input
                    hx-get="/plot"
                    hx-target="#dms-table-container"
                    hx-trigger="load delay:0.5s, click"
                    hx-include="[name='protein'],[name='condition'],[name='position_filter'],[name='paint'],[name='threshold']"
                    type="radio"
                    name="plot"
                    value="heatmap"
                    id="heatmap"
                    checked
                    {}
                label for="heatmap"{"View Heatmap"}
                input
                    hx-get="/plot"
                    hx-target="#dms-table-container"
                    hx-trigger="click"
                    hx-include="[name='protein'],[name='condition'],[name='position_filter'],[name='paint'],[name='threshold']"
                    type="radio"
                    name="plot"
                    value="scatter"
                    id="scatter"{}
                label for="scatter" {"View Scatterplot"}
                }


            form class="selection-form"
                hx-get="/proteins"
                hx-trigger="load"
                {
                }
        }
        div id="full-view"{
            div id="dms-table-container"
                hx-get="/plot?plot=heatmap"
                hx-include="[name='protein'],[name='condition'],[name='position_filter'],[name='paint'],[name='threshold'],[name='plot']"
                hx-trigger="load delay:0.5s"{
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
                        {
                            @for pos in 1..100{ // just to show content while stuff is loading
                                tr{
                                    th scope="row"{(pos)}
                                    @for amino_acid in &AMINO_ACIDS{
                                        (format_variant_cell(None, &pos, amino_acid, None, None))
                                    }
                                }

                            }

                        }
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
                p id="loading-cell-indicator" class="htmx-indicator" {"Loading..."}
            }
            div id="structure"{}
        }

    };
    base(var_name)
}

async fn get_plot(
    State(state): State<AppState>,
    Query(params): Query<TableParams>,
) -> impl IntoResponse {
    match params.plot {
        Some(plot_type) => match plot_type {
            dms_viewer::PlotType::Scatter => get_scatter_plot(state, params).await.into_response(),
            dms_viewer::PlotType::Heatmap => {
                let mut res = get_heatmap().await.into_response();
                res.headers_mut().insert(
                    "HX-Trigger-After-Settle",
                    HeaderValue::from_static("load-condition"),
                );
                return res;
            }
        },
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            html!("Plot Type enum missing"),
        )
            .into_response(),
    }
}

async fn get_heatmap() -> impl IntoResponse {
    (html!(
        table id="dms-table"{
            thead{
                tr{
                    th{" "}
                    @for amino_acid in &AMINO_ACIDS{
                        th { (amino_acid)}
                    }
                }
            }
        tbody id="dms-table-body"
            hx-get="/variants"
            hx-include="[name='protein'],[name='condition'],[name='position_filter'],[name='paint'],[name='threshold']"
            hx-trigger="load-condition from:body delay:0.25s"
        {
            @for pos in 1..100{ // just to show content while stuff is loading
                tr{
                    th scope="row"{(pos)}
                    @for amino_acid in &AMINO_ACIDS{
                        (format_variant_cell(None, &pos, amino_acid, None, None))
                    }
                }

            }

        }
    }))
    .into_response()
}

async fn get_scatter_plot(state: AppState, params: TableParams) -> impl IntoResponse {
    let TableParams {
        ref protein,
        ref condition,
        page: _,
        position_filter: _,
        ref paint,
        operation: _,
        threshold,
        plot: _,
    } = params;
    let pool = &state.pool;

    let variants = sqlx::query_as!(
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
        "#,
        protein,
        condition,
    )
    .fetch_all(&state.pool)
    .await
    .unwrap();

    if let Some(max_abs) = get_max_absolute_value(protein, condition, *paint, pool).await {
        if let Some(threshold) = threshold {}
        if let Some(min_max) =
            get_range_of_variant(protein, condition, Paint::Log2FoldChange, pool).await
        {
            let normalizer = Normalizer { max_abs };

            let pos_color_pairs: Vec<VariantColor> = variants
                .iter()
                .map(|variant| VariantColor {
                    id: variant.id.unwrap(),
                    pos: variant.pos,
                    color: {
                        match *paint {
                            Paint::Log2FoldChange => {
                                normalizer.get_color_hex(variant.log2_fold_change)
                            }
                            Paint::PValue => normalizer.get_color_hex(variant.p_value),
                            Paint::ZStatistic => normalizer.get_color_hex(variant.statistic),
                        }
                    },
                    aa: variant.aa.clone(),
                    log2_fold_change: variant.log2_fold_change,
                    log2_std_error: variant.log2_std_error,
                    statistic: variant.statistic,
                    p_value: {
                        let neg_log10_p = -variant.p_value.log10();
                        neg_log10_p.min(20.0) // Cap at 21 if greater
                    },
                })
                .collect();

            return html!(
                #container
                    x-data="scatterPlot()"
                    x-init=(format!("initPlot({},{},0,21); setData({})",min_max.min,min_max.max, serde_json::to_string(&pos_color_pairs).unwrap())) {}
            );
        }
    }
    return html!();
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
        Ok(conditions) => {
            let mut res = (html! {
                @for condition in &conditions{
                    option value=(condition.condition) { (condition.condition) }
                }
                script {
                    (PreEscaped(format!("refresh_and_load_pdb_into_viewer('{}')",pdb_id)))
                }
            })
            .into_response();

            res.headers_mut().insert(
                "HX-Trigger-After-Settle",
                HeaderValue::from_static("load-condition"),
            );
            return res;
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            html! {
                p { "Error fetching conditions: " (err) }
            },
        )
            .into_response(),
    }
}

async fn get_proteins(State(state): State<AppState>) -> impl IntoResponse {
    info!("getting proteins");
    let rows = sqlx::query!("SELECT DISTINCT id, name FROM protein;")
        .fetch_all(&state.pool)
        .await;
    match rows {
        Ok(proteins) => {
            let mut res = (html! {
            form class="selection-form"{
                #protein-select-div .select-div{
                    label for="protein" id="protein-select-label"{"Protein"}
                    select id="protein-select" name="protein"
                        hx-get="/conditions"
                        hx-include="this"
                        hx-target="#condition-select"
                        hx-trigger="change,load"

                        {
                            @for protein in &proteins{
                                option value=(protein.name) { (protein.name) }
                            }
                        }
                }
            }
            form
                hx-get="/plot"
                hx-indicator="#loading"
                hx-include="[name='protein'],[name='condition'],[name='position_filter'],[name='paint'],[name='threshold'],[name='plot']"
                hx-target="#dms-table-container"
                hx-trigger="input throttle:0.15s"
            {
                div class="selection-form"
                    hx-get="/threshold"
                    hx-trigger="change, load-condition from:body delay:0.25s"
                    hx-include="[name='protein'],[name='condition'],[name='position_filter'],[name='paint']"
                    hx-target="#threshold-slider"
                {
                    #condition-select-div .select-div{
                        label for="condition" id="condition-select-label"{"Condition"}
                        select id="condition-select" name="condition"
                        {}
                    }

                    #position-filter-select-div .select-div{
                        label id="label-position-filter-select" for="position_filter"{"Select"}
                        select id="position-filter-select" name="position_filter"
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

                        {
                            option value=("Log2FoldChange") { ("log2 Fold Change") }
                            option value=("PValue") { ("p value") }
                            option value=("ZStatistic") { ("z statistic") }
                        }
                    }
                    #threshold
                        name="threshold"
                        x-data="{ threshold_value: 0 }"
                        {
                        #threshold-slider .select-div{
                            label for="threshold"{"Threshold"}

                        }

                    }
                }
            }




            div id="loading" class="htmx-indicator"{"Loading..."}

            }).into_response();
            res.headers_mut().insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("max-age=100"),
            );
            return res;
        }

        Err(err) => (html! {
            p { "Error fetching proteins: " (err) }
        })
        .into_response(),
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
        operation: _,
        ref threshold,
        plot: _,
    } = params;
    let page = page.unwrap_or(1);
    info!(
        "Getting variant for protein = {}, condition = {} and page = {} and order={:?}",
        protein, condition, page, position_filter
    );
    let page_start = (page - 1) * PAGE_SIZE + 1;
    let mut page_end = page_start + PAGE_SIZE;
    match sqlx::query!(
        r#"
        SELECT max(pos) as maximum FROM variant
        JOIN protein ON variant.protein_id = protein.id
        WHERE protein.name = $1
        AND variant.condition = $2
        "#,
        protein,
        condition
    )
    .fetch_one(&state.pool)
    .await
    {
        Ok(record) => match record.maximum {
            Some(maximum) => {
                if page_end >= maximum {
                    page_end = maximum;
                }
            }
            None => {
                warn!("Could not find maximum")
            }
        },
        Err(_) => {
            warn!("Could not find maximum");
        }
    }
    let variants: Vec<Variant> = match position_filter {
        PositionFilter::NoOrder => match threshold {
            Some(threshold) => sqlx::query_as!(
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
                AND case $5
                    when 'p_value' then variant.p_value < $6
                    when 'log2_fold_change' then variant.log2_fold_change < $6
                    when 'statistic' then variant.statistic < $6
                end
                AND variant.pos >= $3
                AND variant.pos <= $4
                ORDER BY variant.pos, variant.aa
                "#,
                protein,
                condition,
                page_start,
                page_end,
                paint.to_string(),
                *threshold
            )
            .fetch_all(&state.pool)
            .await
            .unwrap(),
            None => sqlx::query_as!(
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
                            AND variant.pos >= $3
                            AND variant.pos <= $4
                            ORDER BY variant.pos, variant.aa
                            "#,
                protein,
                condition,
                page_start,
                page_end
            )
            .fetch_all(&state.pool)
            .await
            .unwrap(),
        },
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
                AND pos >= $3
                AND pos <= $4;
                "#,
            protein,
            condition,
            page_start,
            page_end,
            position_filter.to_string()
        )
        .fetch_all(&state.pool)
        .await
        .unwrap(),
    };
    if variants.is_empty() {
        info!("no variants found... perhaps a missing chunk? or a really low threshold")
    }
    let query_length: usize = variants.len();
    info!("{}", query_length);
    let positions: Vec<i32> = (page_start..page_end).collect();
    debug!("{:?}", &positions);
    if let Some(max_abs) = get_max_absolute_value(protein, condition, *paint, &state.pool).await {
        let normalizer = Normalizer { max_abs };
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
                                    @let end_of_row = (pos == &(page_end - 15)) && (&amino_acid == &AMINO_ACIDS.last().unwrap());
                                    (get_variant_cell(&variants, amino_acid, pos, &params, end_of_row,&normalizer))
                                }
                            }
                        }
                        script {(PreEscaped(format!("colorVariants({})",serde_json::json!(pos_color_pairs))))}


                    )
                    ).into_response();
        res.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("max-age=100"),
        );
        return res;
    } else {
        warn!("error");
        (StatusCode::INTERNAL_SERVER_ERROR, html!(div{"eeeee"})).into_response()
    }
}
async fn get_max_absolute_value(
    protein: &str,
    condition: &str,
    paint: Paint,
    pool: &PgPool,
) -> Option<f64> {
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
    .fetch_one(pool)
    .await
    {
        Ok(max_abs_option) => max_abs_option,
        Err(_) => None,
    }
}

struct MinMax {
    min: f64,
    max: f64,
}

async fn get_range_of_variant(
    protein: &str,
    condition: &str,
    paint: Paint,
    pool: &PgPool,
) -> Option<MinMax> {
    match sqlx::query!(
        r#"
                select
                    max(
                        case $3
                            when 'p_value' then variant.p_value
                            when 'log2_fold_change' then variant.log2_fold_change
                            when 'statistic' then variant.statistic
                        end
                    ) as max,
                    min(
                        case $3
                            when 'p_value' then variant.p_value
                            when 'log2_fold_change' then variant.log2_fold_change
                            when 'statistic' then variant.statistic
                        end
                    ) as min
                from variant
                join protein on variant.protein_id = protein.id
                where protein.name = $1 and variant.condition = $2;"#,
        protein,
        condition,
        paint.to_string()
    )
    .fetch_one(pool)
    .await
    {
        Ok(max_abs_option) => Some(MinMax {
            min: max_abs_option.min?,
            max: max_abs_option.max?,
        }),
        Err(_) => None,
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
                    class="dms-cell-data dms-cell"
                    title=(format!("log2FC: {:.3}, {}{}",log2_fc,pos,amino_acid))
                    hx-get=(format!("/variant/{}",variant_id))
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
        class="dms-cell dms-cell-no-data"
        title=(format!("log2FC: {:.3}, {}{}","N/A",pos,amino_acid)){});
}

fn format_invisible_lazy_load_cell(params: &TableParams) -> Markup {
    return html!(
        td
        id="invisible-lazy-load-cell"
        hx-trigger="intersect once"
        hx-target="#dms-table-body"
        hx-indicator="#loading-cell-indicator"
        hx-include="[name='protein'],[name='condition'],[name='position_filter'],[name='paint'],[name='threshold']"
        hx-get=(format!("/variants?page={}",params.page.unwrap_or(1)+1))
        hx-swap="beforeend"
        {});
}

fn upload_file_component_with_message(message: &str) -> Markup {
    info!("IN");
    html! {
        div id="upload-file-message" {(message)}
        p id="upload-indicator" class="htmx-indicator" {"Uploading file..."}
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
                        res.headers_mut().insert(
                            "HX-Trigger-After-Settle",
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
        res.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("max-age=100"),
        );
        return res;
    } else {
        warn!("Variant not found!");
        return html!("Variant not found!").into_response();
    }
}

#[derive(Deserialize)]
struct IdQuery {
    ids: Vec<i32>, // Vec<i32> to hold the list of IDs
}

// The handler function that takes an array of IDs
async fn get_many_variants_by_id(Query(query): Query<IdQuery>) -> impl IntoResponse {
    // You can access the IDs via query.ids
    // Example of accessing the first id:
    if !query.ids.is_empty() {
        println!("First ID: {}", query.ids[0]);
    }

    // Handle your logic here, e.g., querying the database with the ids
    let response = format!("Received IDs: {:?}", query.ids);

    (response).into_response()
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

async fn get_threshold_for_paint_by(
    State(state): State<AppState>,
    Query(params): Query<TableParams>,
) -> impl IntoResponse {
    let TableParams {
        ref protein,
        ref condition,
        page: _,
        position_filter,
        ref paint,
        operation: _,
        threshold: _,
        plot: _,
    } = params;
    let pool = &state.pool;
    match position_filter {
        PositionFilter::NoOrder => {
            let rows = sqlx::query!(
                r#"
                select
                    max(
                        case $3
                            when 'p_value' then variant.p_value
                            when 'log2_fold_change' then variant.log2_fold_change
                            when 'statistic' then variant.statistic
                        end
                    ) as "max_value!",
                    min(
                        case $3
                            when 'p_value' then variant.p_value
                            when 'log2_fold_change' then variant.log2_fold_change
                            when 'statistic' then variant.statistic
                        end
                    ) as "min_value!"
                from variant
                join protein on variant.protein_id = protein.id
                where protein.name = $1 and variant.condition = $2;
                "#,
                protein,
                condition,
                paint.to_string()
            )
            .fetch_one(pool)
            .await;
            match rows {
                Ok(row) => {
                    let step = (row.max_value - row.min_value) / 50.0;
                    return (html!(
                        label for="threshold"{"Threshold"}
                            div style="display: flex; align-items: center;" {
                                input
                                    type="range"
                                    id="threshold"
                                    name="threshold"
                                    min=(format!("{:.3}",row.min_value))
                                    max=(format!("{:.3}",row.max_value))
                                    step=(format!("{:.3}",step))
                                    x-model="threshold_value"
                                    {}
                                div style="margin-left: 10px;" x-text="threshold_value"{}
                            }


                    ))
                    .into_response();
                }
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        html!(div{(format!("no max or min found, {}",e))}),
                    )
                        .into_response();
                }
            }
        }
        _ => (html!()).into_response(),
    }
}
#[derive(Debug, serde::Deserialize)]
pub struct TitleQuery {
    previous: Option<String>,
}

async fn get_title(Query(query): Query<TitleQuery>) -> impl IntoResponse {
    let titles = vec![
        "DeepScan",
        "A Scanner Deeply",
        "DMV",
        "Twenty Thousand Leagues Under the Sea",
        "VESPA", // Visualize Effects of Site-Specific Protein Alterations
    ];
    let filtered_titles: Vec<&str> = match &query.previous {
        Some(prev) => titles.into_iter().filter(|title| title != prev).collect(),
        None => titles.into_iter().collect(),
    };

    let mut rng = rand::thread_rng();

    let random_number: usize = rng.gen_range(0..filtered_titles.len());
    let new_title = filtered_titles.get(random_number).unwrap();
    return html!(
        span
            hx-get=(format!("/title?previous={new_title}"))
            hx-trigger="every 5s"
            hx-swap="outerHTML swap:1s settle:1s"
            id="page-title-end" {(new_title.to_uppercase())}
    )
    .into_response();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Welcome to DeepScan!");
    let state = AppState::from_env().await?;
    let listener =
        TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, state.env.port))).await?;
    info!("loaded");
    let app = Router::new()
        .route("/", get(main_content))
        .route("/variants", get(get_variants))
        .route("/plot", get(get_plot))
        // .route("/heatmap", get(get_heatmap))
        .route("/proteins", get(get_proteins))
        .route("/conditions", get(get_conditions))
        // .route("/upload", post(upload_file))
        .route("/variant/:id", get(get_variant_by_id))
        .route("/variant", get(get_many_variants_by_id))
        .route("/threshold", get(get_threshold_for_paint_by))
        .route("/title", get(get_title))
        // .route("/scatter", get(get_scatter_plot))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 100000))
        .with_state(state)
        .nest_service(
            "/assets",
            ServiceBuilder::new()
                .layer(middleware::from_fn(set_static_cache_control))
                .service(
                    ServeDir::new("assets")
                        .precompressed_br()
                        .precompressed_gzip(),
                ),
        );
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
    Ok(())
}
