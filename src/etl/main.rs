use chrono::DateTime;
use dms_viewer::Variant;
use sqlx::{postgres::PgPoolOptions, query};

fn read_tsv(file_path: &str, protein: &str) -> Vec<Variant> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t') // Specify TSV format
        .from_path(file_path)
        .unwrap();
    let created_at =
        DateTime::from_timestamp(chrono::Utc::now().timestamp(), 0).expect("timestamp failed");
    let variants = reader
        .deserialize()
        .map(|res| {
            let mut val: Variant = res.unwrap();
            val.protein = protein.to_string();
            val.created_at = created_at;
            return val;
        })
        .collect::<Vec<Variant>>();
    variants
}

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    let file_path = "./data/GLP1R-rerun-combined-cleaned.sumstats.tsv";
    let variants = read_tsv(file_path, "GLP1R");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://postgres@localhost/db")
        .await?;
    let mut txn = pool.begin().await?;
    for variant in variants {
        println!("{:?}", variant);
        // query!("INSERT INTO dms (chunk, pos,condition,aa,log2_fold_change,log2_std_error,statistic,p_value,version,protein,created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
            &variant.chunk,
            variant.pos,
            variant.condition,
            &variant.aa,
            variant.log2_fold_change,
            variant.log2_std_error,
            variant.statistic,
            variant.p_value,
            variant.version,
            variant.protein,
            variant.created_at)
            .execute(&mut *txn)
            .await?;
    }
    txn.commit().await?;

    Ok(())
}
