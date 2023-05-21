pub mod api;
pub mod flow;
mod schema;

use super::state::AppState;
use axum::{
    routing::{delete, get, post},
    Router,
};

pub fn create_authentication_router() -> Router<AppState> {
    Router::new()
        .route("/twitch", get(flow::authenticate_twitch))
        .route("/twitch/bot", get(flow::admin_authenticate_twitch_bot))
        .route("/twitch/manage", get(flow::authenticate_twitch_manage))
        .route("/twitch/redirect", get(flow::twitch_redirect))
        .route("/twitch/redirect/bot", get(flow::admin_twitch_bot_redirect))
        .route("/twitch/redirect/manage", get(flow::twitch_manage_redirect))
        .route("/discord", get(flow::authenticate_discord))
        .route("/discord/redirect", get(flow::discord_redirect))
        .route("/spotify", get(flow::authenticate_spotify))
        .route("/spotify/redirect", get(flow::spotify_redirect))
}

pub fn create_session_router() -> Router<AppState> {
    Router::new()
        .route("/", get(api::get_session))
        .route("/user", get(api::get_user))
        .route("/logout", post(api::logout))
        .route("/lastfm", post(api::set_lastfm_name))
        .route("/spotify", delete(api::disconnect_spotify))
}
