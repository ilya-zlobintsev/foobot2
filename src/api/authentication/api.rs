use rocket::{http::Status, serde::json::Json, State};
use rocket_okapi::openapi;

use super::schema::{get_user_info, UserInfo};
use crate::{
    api::error::ApiError,
    command_handler::CommandHandler,
    database::models::{User, WebSession},
};

#[openapi(tag = "Session")]
#[get("/")]
pub async fn get_session(web_session: WebSession) -> Json<WebSession> {
    Json(web_session)
}

// Not documented because schemas can't be generated for external types
#[openapi(skip)]
#[get("/user")]
pub async fn get_user(cmd: &State<CommandHandler>, user: User) -> Result<Json<UserInfo>, ApiError> {
    let user_info = get_user_info(cmd, user).await?;
    Ok(Json(user_info))
}

#[openapi(tag = "Session")]
#[post("/logout")]
pub async fn logout(cmd: &State<CommandHandler>, web_session: WebSession) {
    cmd.db
        .remove_web_session(&web_session.session_id)
        .expect("DB Error");
}

#[openapi(tag = "Session")]
#[post("/lastfm", data = "<name>")]
pub async fn set_lastfm_name(
    web_session: WebSession,
    cmd: &State<CommandHandler>,
    name: String,
) -> Status {
    cmd.db
        .set_lastfm_name(web_session.user_id, &name)
        .expect("DB Error");

    Status::Accepted
}

#[openapi(tag = "Session")]
#[delete("/spotify")]
pub async fn disconnect_spotify(session: WebSession, cmd: &State<CommandHandler>) {
    cmd.db
        .remove_user_data(session.user_id, "spotify_access_token")
        .expect("DB error");
    cmd.db
        .remove_user_data(session.user_id, "spotify_refresh_token")
        .expect("DB error");
}
