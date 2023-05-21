use crate::command_handler::CommandHandler;
use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use dashmap::DashMap;
use std::sync::Arc;

#[derive(Clone, FromRef)]
pub struct AppState {
    pub cmd: CommandHandler,
    pub state_storage: Arc<DashMap<String, String>>,
    pub http_client: reqwest::Client,
    pub secret_key: Key,
    pub raw_secret_key: String,
}
