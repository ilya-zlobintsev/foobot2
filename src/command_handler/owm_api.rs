use std::sync::Arc;

use http::status::StatusCode;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct OwmApi {
    client: Client,
    api_key: Arc<String>,
}

impl OwmApi {
    pub fn init(api_key: String) -> Self {
        Self {
            api_key: Arc::new(api_key),
            client: Client::new(),
        }
    }

    pub async fn get_current(&self, place: &str) -> Result<WeatherResponse, OwmError> {
        let response = self
            .client
            .get("https://api.openweathermap.org/data/2.5/weather?")
            .query(&[("q", place), ("appid", &self.api_key), ("units", "metric")])
            .send()
            .await?;

        tracing::info!("GET {}: {}", response.url(), response.status());

        match response.status() {
            StatusCode::OK => Ok(response.json().await?),
            StatusCode::NOT_FOUND => Err(OwmError::LocationNotFound),
            _ => Err(OwmError::UnexpectedCode(response.status().to_string())),
        }
    }
}

pub enum OwmError {
    ReqwestError(reqwest::Error),
    DeserializeError(serde_json::Error),
    LocationNotFound,
    UnexpectedCode(String),
}

impl From<reqwest::Error> for OwmError {
    fn from(e: reqwest::Error) -> Self {
        Self::ReqwestError(e)
    }
}

impl From<serde_json::Error> for OwmError {
    fn from(e: serde_json::Error) -> Self {
        Self::DeserializeError(e)
    }
}

impl std::fmt::Display for OwmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&match self {
            OwmError::ReqwestError(e) => format!("reqwest error: {}", e),
            OwmError::DeserializeError(e) => format!("parser error: {}", e),
            OwmError::LocationNotFound => "location not found".to_string(),
            OwmError::UnexpectedCode(code) => format!("unexpected code {}", code),
        })
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherResponse {
    pub base: String,
    pub clouds: Clouds,
    pub cod: i64,
    pub coord: Coord,
    pub dt: i64,
    pub id: i64,
    pub main: Main,
    pub name: String,
    pub sys: Sys,
    pub timezone: i64,
    pub visibility: i64,
    pub weather: Vec<Weather>,
    pub wind: Wind,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clouds {
    pub all: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coord {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Main {
    #[serde(rename = "feels_like")]
    pub feels_like: f64,
    pub humidity: i64,
    pub pressure: i64,
    pub temp: f64,
    #[serde(rename = "temp_max")]
    pub temp_max: f64,
    #[serde(rename = "temp_min")]
    pub temp_min: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sys {
    pub country: Option<String>,
    pub id: Option<i64>,
    pub sunrise: i64,
    pub sunset: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Weather {
    pub description: String,
    pub icon: String,
    pub id: i64,
    pub main: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wind {
    pub deg: i64,
    // pub gust: f64,
    pub speed: f64,
}
