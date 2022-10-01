use serde::Serialize;

use crate::{
    api::error::ApiError,
    command_handler::{twitch_api, CommandHandler},
    database::models::User,
};

#[derive(Serialize)]
pub struct UserInfo {
    #[serde(flatten)]
    pub base_user: User,
    pub twitch_user: Option<twitch_api::model::User>,
    pub discord_user: Option<twilight_model::user::User>,
    pub admin: bool,
    pub lastfm_name: Option<String>,
    pub spotify_connected: bool,
}

pub async fn get_user_info(cmd: &CommandHandler, user: User) -> Result<UserInfo, ApiError> {
    let admin = if let Some(admin) = cmd.db.get_admin_user()? {
        admin.id == user.id
    } else {
        false
    };

    let platform_handler = cmd.platform_handler.read().await;

    let twitch_user = match (&user.twitch_id, platform_handler.twitch_api.as_ref()) {
        (Some(twitch_id), Some(twitch_api)) => Some(
            twitch_api
                .helix_api
                .get_user_by_id(twitch_id)
                .await
                .unwrap_or_else(|error| {
                    tracing::error!("Failed to query Twitch user: {error}");
                    twitch_api::model::User {
                        id: twitch_id.clone(),
                        ..Default::default()
                    }
                }),
        ),
        _ => None,
    };

    let discord_user = match (&user.discord_id, platform_handler.discord_api.as_ref()) {
        (Some(discord_id), Some(discord_api)) => {
            Some(discord_api.get_user(discord_id.parse().unwrap()).await?)
        }
        _ => None,
    };

    let lastfm_name = cmd.db.get_lastfm_name(user.id)?;

    let spotify_connected = cmd.db.get_spotify_access_token(user.id)?.is_some();

    Ok(UserInfo {
        base_user: user,
        twitch_user,
        discord_user,
        admin,
        lastfm_name,
        spotify_connected,
    })
}
