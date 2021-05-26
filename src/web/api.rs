use rocket::response::Responder;
use rocket::response::{self};
use rocket::{
    get,
    http::{ContentType, CookieJar, Status},
    Response, State,
};
use serenity::model::id::{ChannelId, GuildId, UserId};

use crate::{
    command_handler::{CommandHandler, DiscordContext},
    platform::{discord, ChannelIdentifier},
};

use super::template_context::AuthInfo;

#[get("/permissions?<channel_id>")]
pub async fn get_permissions(
    channel_id: &str,
    jar: &CookieJar<'_>,
    cmd: &State<CommandHandler>,
) -> Result<String, ApiError> {
    let db = &cmd.db;

    match AuthInfo::new(db, jar) {
        Some(auth_info) => match db.get_channel_by_id(channel_id.parse().expect("Invalid ID"))? {
            Some(channel) => match ChannelIdentifier::new(&channel.platform, channel.channel)? {
                ChannelIdentifier::TwitchChannelName(twitch_channel) => {
                    let twitch_id = db
                        .get_user_by_id(auth_info.user_id)?
                        .ok_or_else(|| ApiError::InvalidUser)?
                        .twitch_id
                        .ok_or_else(|| {
                            ApiError::GenericError("No registered on this platform".to_string())
                        })?;

                    let twitch_api = cmd.twitch_api.as_ref().ok_or_else(|| {
                        ApiError::GenericError("Twitch not configured".to_string())
                    })?;

                    match twitch_api
                        .get_channel_mods(&twitch_channel)
                        .await?
                        .contains(
                            &twitch_api
                                .get_users(None, Some(&vec![&twitch_id]))
                                .await?
                                .first()
                                .unwrap()
                                .display_name,
                        ) {
                        true => Ok("channel_mod".to_owned()),
                        false => Ok("none".to_owned()),
                    }
                }
                ChannelIdentifier::DiscordGuildID(guild_id) => {
                    let discord_user_id = db
                        .get_user_by_id(auth_info.user_id)?
                        .ok_or_else(|| ApiError::InvalidUser)?
                        .discord_id
                        .ok_or_else(|| {
                            ApiError::GenericError("No registered on this platform".to_string())
                        })?;

                    let discord_context: DiscordContext =
                        (**cmd.discord_context.as_ref().ok_or_else(|| {
                            ApiError::GenericError("Discord not configured".to_string())
                        })?)
                        .clone();

                    let permissions = discord::get_permissions_in_guild(
                        discord_context,
                        GuildId(guild_id),
                        None,
                        UserId(discord_user_id.parse().unwrap()),
                    )
                    .await;

                    Ok(permissions.to_string())
                }
                _ => unimplemented!(),
            },
            None => Ok("none".to_owned()),
        },
        None => Ok("none".to_owned()),
    }
}

pub enum ApiError {
    InvalidUser,
    DatabaseError(diesel::result::Error),
    RequestError(reqwest::Error),
    GenericError(String),
}

impl From<diesel::result::Error> for ApiError {
    fn from(e: diesel::result::Error) -> Self {
        Self::DatabaseError(e)
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(e: reqwest::Error) -> Self {
        Self::RequestError(e)
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self::GenericError(e.to_string())
    }
}

impl<'a> Responder<'a, 'a> for ApiError {
    fn respond_to(self, _: &'a rocket::Request<'_>) -> response::Result<'static> {
        Response::build()
            .status(Status::NotFound)
            .header(ContentType::JSON)
            .ok()
    }
}
