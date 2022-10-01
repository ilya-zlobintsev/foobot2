use std::collections::HashMap;

use reqwest::{header::HeaderMap, Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone)]
pub struct SpotifyApi {
    client: reqwest::Client,
    headers: HeaderMap,
}

impl SpotifyApi {
    pub fn new(access_token: &str) -> SpotifyApi {
        let mut headers = HeaderMap::new();

        headers.append(
            "Authorization",
            format!("Bearer {}", access_token).parse().unwrap(),
        );

        SpotifyApi {
            client: Client::new(),
            headers,
        }
    }
    pub async fn get_current_song(&self) -> Result<Option<CurrentPlayback>, reqwest::Error> {
        tracing::info!("Getting current song");

        let response = self
            .client
            .get("https://api.spotify.com/v1/me/player")
            .headers(self.headers.clone())
            .send()
            .await?;

        tracing::info!("GET {}: {}", response.url(), response.status());

        match response.status() {
            StatusCode::OK => Ok(response.json().await?),
            StatusCode::NO_CONTENT => Ok(None),
            _ => unimplemented!(),
        }
    }

    pub async fn get_recently_played(&self) -> Result<String, reqwest::Error> {
        match self
            .client
            .get("https://api.spotify.com/v1/me/player/recently-played")
            .headers(self.headers.clone())
            .send()
            .await?
            .json::<Value>()
            .await
        {
            Ok(recently_played) => {
                let last_track = &recently_played["items"][0]["track"];

                let artist = last_track["artists"][0]["name"].as_str().unwrap();
                let song = last_track["name"].as_str().unwrap();

                Ok(format!("{} - {}", artist, song))
            }
            Err(e) => Ok(format!("error getting last song: {:?}", e)),
        }
    }

    /// Returns the new access token and the expiration time
    pub async fn update_token(
        http_client: &Client,
        client_id: &str,
        client_secret: &str,
        refresh_token: &str,
    ) -> Result<(String, u64), reqwest::Error> {
        let mut payload: HashMap<&str, &str> = HashMap::new();
        payload.insert("grant_type", "refresh_token");
        payload.insert("refresh_token", refresh_token);
        payload.insert("redirect_uri", "http://localhost:5555/");
        payload.insert("client_id", client_id);
        payload.insert("client_secret", client_secret);

        let response = http_client
            .post("https://accounts.spotify.com/api/token")
            .form(&payload)
            .send()
            .await?
            .json::<Value>()
            .await?;

        Ok((
            response["access_token"].as_str().unwrap().to_string(),
            response["expires_in"].as_u64().unwrap(),
        ))
    }

    /// Returns access and refresh tokens
    pub async fn get_tokens(
        code: &str,
        client_id: &str,
        client_secret: &str,
        redirect_uri: &str,
    ) -> Result<SpotifyAuthentication, reqwest::Error> {
        let client = Client::new();

        let mut payload = HashMap::new();

        payload.insert("grant_type", "authorization_code");
        payload.insert("code", code);
        payload.insert("redirect_uri", redirect_uri);
        payload.insert("client_id", client_id);
        payload.insert("client_secret", client_secret);

        let response = client
            .post("https://accounts.spotify.com/api/token")
            .form(&payload)
            .send()
            .await?;

        tracing::info!("POST {}: {}", response.url(), response.status());

        response.json().await
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyAuthentication {
    #[serde(rename = "access_token")]
    pub access_token: String,
    #[serde(rename = "token_type")]
    pub token_type: String,
    pub scope: String,
    #[serde(rename = "expires_in")]
    pub expires_in: i64,
    #[serde(rename = "refresh_token")]
    pub refresh_token: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentPlayback {
    pub context: Option<Context>,
    #[serde(rename = "progress_ms")]
    pub progress_ms: i64,
    pub item: Item,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    #[serde(rename = "external_urls")]
    pub external_urls: ExternalUrls,
    pub href: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub uri: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalUrls {
    pub spotify: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub album: Album,
    pub artists: Vec<Artist>,
    #[serde(rename = "duration_ms")]
    pub duration_ms: i64,
    #[serde(rename = "is_local")]
    pub is_local: bool,
    pub name: String,
    pub uri: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub artists: Vec<Artist>,
    pub href: String,
    pub id: String,
    pub name: String,
    #[serde(rename = "total_tracks")]
    pub total_tracks: i64,
    #[serde(rename = "type")]
    pub type_field: String,
    pub uri: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub href: String,
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub uri: String,
}
