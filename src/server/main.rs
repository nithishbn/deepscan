use std::io::Cursor;

use axum::{
    body::{Body, Bytes},
    debug_handler,
    extract::{DefaultBodyLimit, Multipart, Query, State},
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use chrono::DateTime;
use csv::Error;
use dms_viewer::Variant;
use futures_util::future::join_all;
use maud::{html, Markup, DOCTYPE};
use serde::Deserialize;
use sqlx::{postgres::PgPoolOptions, query, query_scalar, PgPool};
use tokio::sync::SetError;
use tracing::{info, Level};

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}
#[derive(Deserialize)]
struct TableParams {
    protein: String,
    // condition: String,
}

const AMINO_ACIDS: [&str; 21] = [
    "*", "A", "C", "D", "E", "F", "G", "H", "I", "K", "L", "M", "N", "P", "Q", "R", "S", "T", "V",
    "W", "Y",
];
fn base(content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=3, maximum-scale=1, user-scalable=no" {}
                title { "DeepScan" }

                // Styles
                link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.3/dist/css/bootstrap.min.css" {}

                // Htmx + Alpine
                script src="https://unpkg.com/htmx.org@1.9.10" {}
                script src="//unpkg.com/alpinejs" defer {}

            }
            body hx-boost="true"{
                div class="container p-3" {
                    (content)
                }
            }
        }
    }
}
#[debug_handler]
async fn hello_world() -> Markup {
    base(html! {
            h1 { "DMS Viewer" }
                form hx-post="/upload" hx-encoding="multipart/form-data" hx-include="[name='protein']"{
                    input type="file" name="file" {}
                    button{ "Upload" }
                }
                form hx-get="/proteins" hx-trigger="load" hx-target="#protein-select" hx-swap="innerHtml"{
                    select id="protein-select" name="protein"
                        hx-get="/variants"
                        hx-include="this"
                        hx-target="#dms-table"
                        hx-trigger="change,load delay:0.1s"
                        hx-swap="innerHtml" {}
               }

            table id="dms-table"{
    }
        })
}

async fn get_proteins(State(state): State<AppState>) -> Markup {
    info!("getting proteins");
    let proteins = sqlx::query!("SELECT DISTINCT id, protein FROM proteins;")
        .fetch_all(&state.pool)
        .await;
    match proteins {
        Ok(rows) => {
            html! {
                    @for row in rows {
                        @if row.protein=="GLP1R"{
                            option value=(row.protein) selected { (row.protein) }
                        }@else{
                            option value=(row.protein) { (row.protein) }
                        }
                    }

            }
        }
        Err(err) => {
            // Handle the error, e.g., log it and return an error message
            html! {
                p { "Error fetching proteins: " (err) }
            }
        }
    }
}

async fn get_variants(State(state): State<AppState>, Query(params): Query<TableParams>) -> Markup {
    info!("Getting variant for {}", params.protein);
    let variants = sqlx::query_as!(
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
            ORDER BY dms.pos, dms.condition
            LIMIT 10000;
            "#,
        params.protein
    )
    .fetch_all(&state.pool)
    .await
    .unwrap();
    match query_scalar!(r#"select max(pos) from (select distinct pos from dms join proteins on dms.protein_id = proteins.id where  proteins.protein = $1);"#,params.protein).fetch_one(&state.pool).await{
        Ok(protein_length_option) =>{
            match protein_length_option{
                Some(protein_length)=>{
                    let positions: Vec<i32> = (1..protein_length).collect();
                    html!(
                        tr{
                            th{}
                            @for pos in &positions{
                                th scope="col"{ (pos)}}
                            }
                        @for name in &AMINO_ACIDS{
                            tr{th scope="row"{(name)}
                                @for pos in &positions{
                                    @for variant in &variants {
                                        @if name == &variant.aa && pos == &variant.pos{
                                            td {
                                                (variant.log2_fold_change)
                                            }
                                        }
                                    }
                                }
                            }
                        })
                },
                None =>{
                    html!(div {"protein not found"})
                }
            }
        },
        Err(err)=>{
            html!(div {"hello"})
        }
    }
}

#[debug_handler]
async fn upload_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Response<Body>, Response<Body>> {
    info!("Uploading file");
    let mut protein: Option<String> = None;
    let mut file: Option<Bytes> = None;
    while let Some(mut field) = multipart.next_field().await.unwrap() {
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
                            Ok(result) => Ok((
                                StatusCode::OK,
                                format!("File successfully uploaded. {result} rows affected"),
                            )
                                .into_response()),
                            Err(err) => Ok((StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
                                .into_response()),
                        }
                        // Return the processed result
                    }
                    Err(_) => {
                        // Handle any error from read_tsv
                        Ok((StatusCode::INTERNAL_SERVER_ERROR, "Error processing file")
                            .into_response())
                    }
                }
            } else {
                // No file uploaded
                Ok((StatusCode::BAD_REQUEST, "No file uploaded").into_response())
            }
        }
        None => {
            // No protein found
            Ok((StatusCode::BAD_REQUEST, "No protein found").into_response())
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
    let variants: Vec<Variant> = reader
        .deserialize::<Variant>()
        .filter_map(|res| {
            res.ok().map(|mut val| {
                val.protein = protein.to_string();
                val.created_at = created_at;
                val
            })
        })
        .collect::<Vec<Variant>>();
    Ok(variants)
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
        .connect("postgres://postgres@localhost/db")
        .await?;
    let state = AppState { pool };
    // run it with hyper on localhost:3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    let app = Router::new()
        .route("/", get(hello_world))
        .route("/variants", get(get_variants))
        .route("/proteins", get(get_proteins))
        .route("/upload", post(upload_file))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 1000))
        .with_state(state);
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
    Ok(())
}
