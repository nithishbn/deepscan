use std::{borrow::Cow, env};

use anyhow::bail;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::{prelude::FromRow, types::chrono::Utc, PgPool};
use tracing::info;

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Variant {
    pub id: Option<i32>,
    pub chunk: i32,
    pub pos: i32,
    pub condition: String,
    pub aa: String,
    #[serde(alias = "log2FoldChange")]
    pub log2_fold_change: f64,
    #[serde(alias = "log2StdError")]
    pub log2_std_error: f64,
    pub statistic: f64,
    #[serde(alias = "p.value")]
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

pub const AMINO_ACIDS: [&str; 21] = [
    "*", "A", "C", "D", "E", "F", "G", "H", "I", "K", "L", "M", "N", "P", "Q", "R", "S", "T", "V",
    "W", "Y",
];
pub const PAGE_SIZE: i32 = 100;

pub struct Normalizer {
    pub max_abs: f64,
}

impl Normalizer {
    // Normalize a single value and return the RGB color
    pub fn get_color_rgb(&self, value: f64) -> String {
        if self.max_abs == 0.0 {
            return "rgb(255,255,255)".to_string(); // Handle case where max_abs is 0
        }

        let normalized = value / self.max_abs;

        if normalized < 0.0 {
            let intensity = ((1.0 + normalized) * 255.0).round() as u8; // Scale [-1, 0] to [0, 255]
            format!("rgb(255,{},{})", intensity, intensity) // Shades of red
        } else {
            let intensity = ((1.0 - normalized) * 255.0).round() as u8; // Scale [0, 1] to [255, 0]
            format!("rgb({},{},255)", intensity, intensity) // Shades of blue
        }
    }

    // Normalize a single value and return the Hex color
    pub fn get_color_hex(&self, value: f64) -> String {
        if self.max_abs == 0.0 {
            info!("max_abs is 0");
            return "#FFFFFF".to_string(); // Handle case where max_abs is 0
        }

        let normalized = value / self.max_abs;

        if normalized < 0.0 {
            let intensity = ((1.0 + normalized) * 255.0).round() as u8; // Scale [-1, 0] to [0, 255]
            format!("#FF{:02X}{:02X}", intensity, intensity) // Shades of red in hex
        } else {
            let intensity = ((1.0 - normalized) * 255.0).round() as u8; // Scale [0, 1] to [255, 0]
            format!("#{:02X}{:02X}FF", intensity, intensity) // Shades of blue in hex
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct PosColor {
    pub pos: i32,
    pub color: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VariantColor {
    pub id: i32,
    pub pos: i32,
    pub aa: String,
    pub log2_fold_change: f64,
    pub log2_std_error: f64,
    pub statistic: f64,
    pub p_value: f64,
    pub color: String,
}

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
                _ => 3000,
            },
        })
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub env: EnvironmentVariables,
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
pub enum PositionFilter {
    MostSignificantPValue,
    LargestLog2FoldChange,
    LargestZStatistic,
    NoOrder,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum Paint {
    #[serde(alias = "p_value")]
    PValue,
    #[serde(alias = "log2_fold_change")]
    Log2FoldChange,
    #[serde(alias = "statistic")]
    ZStatistic,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Operation {
    Mean,
    Maximum,
    Minimum,
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
pub struct TableParams {
    pub protein: String,
    pub condition: String,
    pub position_filter: PositionFilter,
    pub paint: Paint,
    pub operation: Option<Operation>,
    pub threshold: Option<f64>,
    pub page: Option<i32>,
}
