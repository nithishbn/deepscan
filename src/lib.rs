use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::{
    prelude::FromRow,
    types::chrono::{DateTime, Utc},
};

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Variant {
    pub id: Option<i32>,
    pub chunk: i32,
    pub pos: i32,
    pub condition: String,
    pub aa: String,
    #[serde(rename = "log2FoldChange")]
    pub log2_fold_change: f64,
    #[serde(rename = "log2StdError")]
    pub log2_std_error: f64,
    pub statistic: f64,
    #[serde(rename = "p.value")]
    pub p_value: f64,
    pub version: String,
    #[serde(default = "default_protein")]
    pub protein: String,
    #[serde(default = "default_timestamp")]
    pub created_on: NaiveDateTime,
}

fn default_timestamp() -> NaiveDateTime {
    Utc::now().naive_utc()
}

fn default_protein() -> String {
    "unknown".to_string()
}
