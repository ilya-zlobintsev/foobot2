use reqwest::Client;
use serde::Deserialize;

const BASE_URL: &str = "https://geohub.vercel.app";
// https://geohub.vercel.app/api/scores/challenges/daily/leaderboard?limit=200

#[derive(Debug, Default)]
pub struct GeohubClient {
    client: Client,
}

impl GeohubClient {
    pub async fn get_leaderboard(&self, limit: u32) -> anyhow::Result<DailyLeaderboard> {
        let url = format!("{BASE_URL}/api/scores/challenges/daily/leaderboard");
        let response = self
            .client
            .get(url)
            .query(&[("limit", limit.to_string())])
            .send()
            .await?
            .json()
            .await?;
        Ok(response)
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
