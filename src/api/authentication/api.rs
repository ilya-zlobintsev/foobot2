use super::schema::{get_user_info, UserInfo};
use crate::{
    api::error::ApiError,
    command_handler::CommandHandler,
    database::models::{User, WebSession},
};
use axum::{extract::State, Json};
use http::StatusCode;

pub async fn get_session(web_session: WebSession) -> Json<WebSession> {
    Json(web_session)
}

pub async fn get_user(cmd: State<CommandHandler>, user: User) -> Result<Json<UserInfo>, ApiError> {
    let user_info = get_user_info(&cmd, user).await?;
    Ok(Json(user_info))
}

pub async fn logout(cmd: State<CommandHandler>, web_session: WebSession) {
    cmd.db
        .remove_web_session(&web_session.session_id)
        .expect("DB Error");
}

pub async fn set_lastfm_name(
    web_session: WebSession,
    cmd: State<CommandHandler>,
    name: String,
) -> StatusCode {
    cmd.db
        .set_lastfm_name(web_session.user_id, &name)
        .expect("DB Error");

    StatusCode::ACCEPTED
}

pub async fn disconnect_spotify(session: WebSession, cmd: State<CommandHandler>) {
    cmd.db
        .remove_user_data(session.user_id, "spotify_access_token")
        .expect("DB error");
    cmd.db
        .remove_user_data(session.user_id, "spotify_refresh_token")
        .expect("DB error");
}
