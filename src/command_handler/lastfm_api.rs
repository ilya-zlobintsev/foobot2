use anyhow::anyhow;
use http::StatusCode;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct LastFMApi {
    client: Client,
    api_key: Arc<String>,
}

impl LastFMApi {
    pub fn init(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key: Arc::new(api_key),
        }
    }

    pub async fn get_recent_tracks(&self, user: &str) -> anyhow::Result<RecentTracksResponse> {
        let response = self
            .client
            .get("https://ws.audioscrobbler.com/2.0/?")
            .query(&[
                ("method", "user.getrecenttracks"),
                ("user", user),
                ("api_key", &*self.api_key),
                ("format", "json"),
            ])
            .send()
            .await?;

        tracing::info!("GET {}: {}", response.url(), response.status());

        match response.status() {
            StatusCode::OK => Ok(response.json().await?),
            status => Err(anyhow!("status code: {}", status)),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentTracksResponse {
    pub recenttracks: Recenttracks,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recenttracks {
    pub track: Vec<Track>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub artist: Artist,
    #[serde(rename = "@attr")]
    pub attr: Option<Attr2>,
    pub mbid: String,
    pub album: Album,
    pub streamable: String,
    pub url: String,
    pub name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub mbid: String,
    #[serde(rename = "#text")]
    pub text: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attr2 {
    pub nowplaying: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub mbid: String,
    #[serde(rename = "#text")]
    pub text: String,
}
