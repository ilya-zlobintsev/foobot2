use std::{sync::Arc, time::Duration};

use anyhow::anyhow;
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::{error, info, instrument, warn};

use crate::database::Database;

use super::platform_handler::PlatformHandler;

const BASE_URL: &str = "https://geohub.vercel.app";

#[derive(Debug, Default)]
pub struct GeohubClient {
    client: Client,
}

impl GeohubClient {
    #[instrument]
    pub async fn get_leaderboard(&self, limit: u32) -> anyhow::Result<DailyLeaderboard> {
        let url = format!("{BASE_URL}/api/scores/challenges/daily/leaderboard");
        let response = self
            .client
            .get(url)
            .query(&[("limit", limit.to_string())])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            warn!("{}", response.text().await?);
            return Err(anyhow!("API error: {status}"));
        }

        Ok(response.json().await?)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyLeaderboard {
    pub all_time: Vec<LeaderboardEntry>,
    pub today: Vec<LeaderboardEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardEntry {
    pub total_time: u32,
    pub total_points: u32,
    pub user_id: String,
    pub user_name: String,
}

pub async fn start_listener(
    db: Database,
    platform_handler: Arc<RwLock<PlatformHandler>>,
    interval: Duration,
    client: GeohubClient,
) -> anyhow::Result<()> {
    let mut last_leaderboard = client.get_leaderboard(200).await?;

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            info!("Fetching new GeoHub leaderboard");

            let links = db.get_geohub_links().expect("DB error");
            match client.get_leaderboard(200).await {
                Ok(new_leaderboard) => {
                    for new_entry in &new_leaderboard.today {
                        // A new entry
                        if !last_leaderboard
                            .today
                            .iter()
                            .any(|entry| entry.user_id == new_entry.user_id)
                        {
                            let entry_name = new_entry.user_name.to_lowercase();

                            if let Some(link) = links
                                .iter()
                                .find(|link| link.geohub_name.to_lowercase() == entry_name)
                            {
                                let channel = db
                                    .get_channel_by_id(link.channel_id)
                                    .expect("DB error")
                                    .expect("Linked to an invalid channel");

                                let message = format!("{} has completed the GeoHub daily challenge with the score of {} points!", new_entry.user_name, new_entry.total_points);
                                if let Err(err) = platform_handler
                                    .read()
                                    .await
                                    .send_to_channel(channel.get_identifier(), message)
                                    .await
                                {
                                    error!("Could not send notification: {err}");
                                }
                            }
                        }
                    }
                    last_leaderboard = new_leaderboard;
                }
                Err(err) => {
                    error!("Could not fetch leaderboard: {err}");
                }
            }
        }
    });

    Ok(())
}
