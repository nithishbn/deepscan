use axum::{routing::get, Router};
use maud::{html, Markup, DOCTYPE};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, FromRow};

#[derive(Debug, FromRow, Serialize, Deserialize)]
struct Variant {
    chunk: i32,
    pos: i32,
    condition: String,
    aa: char,
    #[serde(rename = "log2FoldChange")]
    log2_fold_change: f32,
    #[serde(rename = "log2StdError")]
    log2_std_error: f32,
    statistic: f32,
    p_value: f32,
    version: String,
    total_bc: i32,
    total_bc_sum: i32,
}
fn base(content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=3, maximum-scale=1, user-scalable=no" {}
                title { "DeepScan (htmx)" }

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
async fn hello_world() -> Markup {
    let file_path = "./data/GLP1R-rerun-combined-cleaned.sumstats.tsv"; // Replace with your actual file path
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t') // Specify TSV format
        .from_path(file_path)
        .unwrap();
    let variants = reader
        .deserialize()
        .map(|res| {
            let val: Variant = res.unwrap();
            return val;
        })
        .filter(|value| value.condition == "semaglutide_2e-10")
        .collect::<Vec<Variant>>();
    let mut total_variants: Vec<char> = variants.iter().map(|variant| variant.aa).collect();
    let mut all_positions: Vec<i32> = variants.iter().map(|variant| variant.pos).collect();
    total_variants.sort();
    total_variants.dedup();

    all_positions.sort();
    all_positions.dedup();
    println!("{:?}", total_variants);
    base(html! {
        h1 { "Hello, World!" }
        p { "hello "}

        table hx-boost="true"{
            tr{
                th{}
            @for pos in &all_positions{
                th scope="col"{ (pos)}}
            }
            @for name in &total_variants{
                tr{th scope="row"{(name)}
                @for pos in &all_positions{
                    @for variant in &variants {
                        @if name == &variant.aa && pos == &variant.pos{
                            td {
                                (variant.log2_fold_change)
                            }
                        }
                    }
                }
        }
            }
        }
    })
}

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    // build our application with a single route
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://postgres@localhost/db")
        .await?;

    // run it with hyper on localhost:3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    let app = Router::new().route("/", get(hello_world));
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
    Ok(())
}
