use anyhow::Context;
use reqwest::Client;
use serde::Deserialize;

const TRIVIA_URL: &str = "https://api.gazatu.xyz/trivia/questions?count=1";

#[derive(Clone)]
pub struct TriviaClient {
    pub client: Client,
}

impl TriviaClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub async fn get_random_trivia(&self) -> anyhow::Result<Trivia> {
        let trivia_response = self
            .client
            .get(TRIVIA_URL)
            .send()
            .await?
            .json::<Vec<Trivia>>()
            .await?;
        trivia_response
            .into_iter()
            .next()
            .context("empty trivia response")
    }
}

#[derive(Deserialize)]
pub struct Trivia {
    pub question: String,
    pub answer: String,
    pub category: String,
}
