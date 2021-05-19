use reqwest::{header::HeaderMap, Client};
use serde::Deserialize;

const API_ENDPOINT: &'static str = "https://discord.com/api";

pub struct DiscordApi {
    client: Client,
    headers: HeaderMap,
}

impl DiscordApi {
    pub fn init(access_token: &str) -> Self {
        let mut headers = HeaderMap::new();

        headers.insert(
            "Authorization",
            format!("Bearer {}", access_token).parse().unwrap(),
        );

        let client = Client::new();

        tracing::info!("Initialized Discord API with headers {:?}", headers);

        Self { headers, client }
    }

    pub async fn get_self_user(&self) -> Result<User, reqwest::Error> {
        let response = self
            .client
            .get(format!("{}/users/@me", API_ENDPOINT))
            .headers(self.headers.clone())
            .send()
            .await?;

        tracing::info!("GET {}: {}", response.url(), response.status());

        response.json().await
    }
}

#[derive(Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub discriminator: String,
}
