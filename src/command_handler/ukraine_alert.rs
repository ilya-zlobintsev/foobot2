use chrono::Utc;
use reqwest::{Client, RequestBuilder};
use serde::Deserialize;
use strum::AsRefStr;

const BASE_URL: &str = "https://siren.pp.ua";

#[derive(Debug, Default)]
pub struct UkraineAlertClient {
    client: Client,
}

impl UkraineAlertClient {
    fn get(&self, path: &str) -> RequestBuilder {
        self.client.get(format!("{BASE_URL}{path}"))
    }

    pub async fn get_alerts(&self) -> Result<Vec<AlertRegion>, reqwest::Error> {
        self.get("/api/v3/alerts").send().await?.json().await
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertRegion {
    pub region_id: String,
    pub region_type: String,
    pub region_name: String,
    pub last_update: chrono::DateTime<Utc>,
    pub active_alerts: Vec<Alert>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub region_id: String,
    pub region_type: String,
    pub last_update: chrono::DateTime<Utc>,
    pub r#type: AlertType,
}

#[derive(Debug, Deserialize, AsRefStr)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlertType {
    Air,
    Artillery,
    UrbanFights,
    Chemical,
    Nuclear,
    Info,
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RegionType {
    State,
    District,
    Community,
    Null,
}
