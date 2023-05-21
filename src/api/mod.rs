mod authentication;
mod channels;
mod error;
mod state;
mod webhooks;

use anyhow::anyhow;
use axum::Router;
use axum_extra::extract::cookie::Key;
use dashmap::DashMap;
use reqwest::{Client, Response};
use std::{env, sync::Arc};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::{self, DefaultOnResponse, TraceLayer},
};
use tracing::{info, Level};

use self::error::ApiError;
use crate::{api::state::AppState, command_handler::CommandHandler};

type Result<T> = std::result::Result<T, ApiError>;

pub async fn run(command_handler: CommandHandler) {
    let state_storage = Arc::new(DashMap::new());
    let raw_secret_key = env::var("SECRET_KEY").expect("Could not read SECRET_KEY");
    let secret_key = Key::from(raw_secret_key.as_bytes());

    let state = AppState {
        cmd: command_handler,
        state_storage,
        http_client: Client::new(),
        raw_secret_key,
        secret_key,
    };

    let authentication_routes = authentication::create_authentication_router();

    let api_routes = Router::new()
        .nest("/session", authentication::create_session_router())
        .nest("/channels", channels::create_router())
        .nest("/hooks", webhooks::create_router());

    let frontend_service =
        ServeDir::new("web/dist").fallback(ServeFile::new("web/dist/index.html"));

    let app = Router::new()
        .nest_service("/", frontend_service)
        .nest("/api", api_routes)
        .nest("/authenticate", authentication_routes)
        .with_state(state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        );

    let server_url = "0.0.0.0:8000";
    info!("Starting web server at {server_url}");

    axum::Server::bind(&server_url.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap()

    // TODO
    /*let shutdown_handle = rocket.shutdown();

    task::spawn(async move {
        shutdown_handle.await;

        if let Some(admin_channel) = get_admin_channel() {
            command_handler
                .platform_handler
                .read()
                .await
                .send_to_channel(
                    admin_channel,
                    format!("Foobot2 {} Shutting down...", crate::get_version()),
                )
                .await
                .expect("Failed to send shutdown message");
        }
    });*/
}

pub fn get_base_url() -> String {
    env::var("BASE_URL").expect("BASE_URL missing!")
}

pub fn response_ok(r: &Response) -> anyhow::Result<()> {
    if r.status().is_success() {
        Ok(())
    } else {
        Err(anyhow!("Non-success response: {}", r.status()))
    }
}
