use crate::command_handler::CommandHandler;
use axum::extract::FromRef;
use dashmap::DashMap;

#[derive(Clone, FromRef)]
pub struct AppState {
    pub cmd: CommandHandler,
    pub state_storage: DashMap<String, String>,
    pub http_client: reqwest::Client,
    pub secret_key: String,
}
