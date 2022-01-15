use anyhow::anyhow;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct FinnhubApi {
    client: Client,
    api_key: Arc<String>,
}

impl FinnhubApi {
    pub fn init(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key: Arc::new(api_key),
        }
    }

    pub async fn quote(&self, symbol: &str) -> anyhow::Result<QuoteResponse> {
        let response = self
            .client
            .get("https://finnhub.io/api/v1/quote")
            .query(&[("symbol", symbol)])
            .header("X-Finnhub-Token", &*self.api_key)
            .send()
            .await?;

        tracing::info!("GET {}: {}", response.url(), response.status());

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow!("response status {}", response.status()))
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct QuoteResponse {
    #[serde(rename = "c")]
    pub current_price: f32,
    #[serde(rename = "d")]
    pub change: Option<f32>,
    #[serde(rename = "dp")]
    pub percent_change: Option<f32>,
}
