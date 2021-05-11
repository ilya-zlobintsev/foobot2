use std::collections::HashMap;

use reqwest::{header::HeaderMap, Client};
use serde::{Serialize, Deserialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResponse {
    #[serde(rename = "client_id")]
    pub client_id: String,
    pub login: String,
    pub scopes: Vec<String>,
    #[serde(rename = "user_id")]
    pub user_id: String,
    #[serde(rename = "expires_in")]
    pub expires_in: i64,
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsersResponse {
    pub data: Vec<User>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub login: String,
    #[serde(rename = "display_name")]
    pub display_name: String,
    #[serde(rename = "type")]
    pub type_field: String,
    #[serde(rename = "broadcaster_type")]
    pub broadcaster_type: String,
    pub description: String,
    #[serde(rename = "profile_image_url")]
    pub profile_image_url: String,
    #[serde(rename = "offline_image_url")]
    pub offline_image_url: String,
    #[serde(rename = "view_count")]
    pub view_count: i64,
    #[serde(rename = "created_at")]
    pub created_at: String,
}

#[derive(Clone)]
pub struct TwitchApi {
    headers: HeaderMap,
    client: Client,
}

impl TwitchApi {
    pub async fn init(oauth: &str) -> Result<Self, reqwest::Error> {
        let oauth = match oauth.strip_prefix("oauth:") {
            Some(res) => res,
            None => oauth,
        };

        let validation = Self::validate_oauth(oauth).await?;

        let mut headers = HeaderMap::new();
        headers.insert("Client-Id", validation.client_id.parse().unwrap());
        headers.insert(
            "Authorization",
            format!("Bearer {}", oauth).parse().unwrap(),
        );

        Ok(TwitchApi {
            headers,
            client: Client::new(),
        })
    }

    pub async fn validate_oauth(oauth: &str) -> Result<ValidationResponse, reqwest::Error> {
        let client = Client::new();
        let response = client
            .get("https://id.twitch.tv/oauth2/validate")
            .header("Authorization", format!("Bearer {}", oauth))
            .send()
            .await?;
        tracing::info!("Validating twitch API token: {}", response.status());
        Ok(response.json().await?)
    }

    pub fn get_oauth(&self) -> &str {
        self.headers
            .get("Authorization")
            .unwrap()
            .to_str()
            .unwrap()
            .strip_prefix("Bearer ")
            .unwrap()
    }

    fn get_client_id(&self) -> &str {
        self.headers.get("Client-Id").unwrap().to_str().unwrap()
    }

    // TODO: merge into get_users that accepts both ids and logins
    pub async fn get_users_by_login(
        &self,
        logins: &Vec<String>,
    ) -> Result<UsersResponse, reqwest::Error> {
        let mut params: Vec<(&str, &str)> = Vec::new();
        for login in logins {
            params.push(("login", login));
        }

        Ok(self
            .client
            .get("https://api.twitch.tv/helix/users")
            .headers(self.headers.clone())
            .query(&params)
            .send()
            .await?
            .json()
            .await?)
    }

    pub async fn get_users_by_id(
        &self,
        ids: &Vec<String>,
    ) -> Result<UsersResponse, reqwest::Error> {
        let mut params: Vec<(&str, &str)> = Vec::new();
        for id in ids {
            params.push(("id", id));
        }

        Ok(self
            .client
            .get("https://api.twitch.tv/helix/users")
            .headers(self.headers.clone())
            .query(&params)
            .send()
            .await?
            .json()
            .await?)
    }

    pub async fn run_ad(
        &self,
        channel_login: &str,
        duration: u8,
    ) -> Result<String, reqwest::Error> {
        let users = self
            .get_users_by_login(&vec![channel_login.to_string()])
            .await?;
        let channel_id = &users.data.first().unwrap().id;

        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("OAuth {}", self.get_oauth()).parse().unwrap(),
        );
        headers.insert("Client", self.get_client_id().to_owned().parse().unwrap());

        let mut payload = HashMap::new();
        // params.insert("channelID", channel_id);
        // params.insert("channelLogin", channel_login.to_owned());
        payload.insert("length", duration.to_string());

        let url = format!(
            "https://api.twitch.tv/v5/channels/{}/commercial",
            channel_id
        );

        Ok(self
            .client
            .post(&url)
            .headers(headers)
            .json(&payload)
            .send()
            .await?
            .text()
            .await?)
    }
}
