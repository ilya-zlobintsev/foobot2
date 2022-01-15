use anyhow::anyhow;
use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;

pub struct LingvaApi {
    client: Client,
    url: Arc<String>,
}

impl LingvaApi {
    pub fn init(instance_url: String) -> Self {
        Self {
            client: Client::new(),
            url: Arc::new(instance_url),
        }
    }

    pub async fn translate(
        &self,
        source: &str,
        target: &str,
        query: &str,
    ) -> anyhow::Result<String> {
        let request_url = format!("{}/api/v1/{}/{}/{}", self.url, source, target, query);

        let response = self.client.get(request_url).send().await?;

        tracing::info!("{}: {}", response.url(), response.status());

        match response.status().is_success() {
            true => {
                let response_object: Value = response.json().await?;

                let translation = response_object
                    .get("translation")
                    .unwrap()
                    .as_str()
                    .unwrap();

                Ok(translation.to_owned())
            }
            false => Err(anyhow!("Translation API Error: {}", response.status())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LingvaApi;

    const TEST_URL: &str = "https://lingva.ml/";

    #[tokio::test]
    async fn test_translate() {
        tracing_subscriber::fmt::init();

        let lingva_api = LingvaApi::init(TEST_URL.to_owned());

        let translation = lingva_api.translate("auto", "ru", "Hello").await;

        assert_eq!(translation.unwrap(), "Привет");
    }
}
