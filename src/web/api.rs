use rocket::get;
use rocket::http::{ContentType, Status};
use rocket::response::Responder;
use rocket::response::{self};
use rocket::{Response, State};

use crate::database::models::WebSession;
use crate::database::DatabaseError;
use crate::platform::Permissions;
use crate::{command_handler::CommandHandler, platform::ChannelIdentifier};

#[post("/user/lastfm", data = "<name>")]
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

#[get("/permissions?<channel_id>")]
pub async fn get_permissions(
    channel_id: &str,
    web_session: WebSession,
    cmd: &State<CommandHandler>,
) -> Result<Permissions, ApiError> {
    let db = &cmd.db;

    match db.get_channel_by_id(channel_id.parse().expect("Invalid ID"))? {
        Some(channel) => match ChannelIdentifier::new(&channel.platform, channel.channel)? {
            ChannelIdentifier::TwitchChannelID(channel_id) => {
                let twitch_id = db
                    .get_user_by_id(web_session.user_id)?
                    .ok_or_else(|| ApiError::InvalidUser)?
                    .twitch_id
                    .ok_or_else(|| {
                        ApiError::GenericError("Not registered on this platform".to_string())
                    })?;

                let twitch_api = cmd
                    .twitch_api
                    .as_ref()
                    .ok_or_else(|| ApiError::GenericError("Twitch not configured".to_string()))?;

                let users_response = twitch_api.get_users(None, Some(&vec![&channel_id])).await?;

                let channel_login = &users_response.first().expect("User not found").login;

                match twitch_api.get_channel_mods(&channel_login).await?.contains(
                    &twitch_api
                        .get_users(None, Some(&vec![&twitch_id]))
                        .await?
                        .first()
                        .unwrap()
                        .display_name,
                ) {
                    true => Ok(Permissions::ChannelMod),
                    false => Ok(Permissions::Default),
                }
            }
            ChannelIdentifier::DiscordGuildID(guild_id) => {
                let user_id = db
                    .get_user_by_id(web_session.user_id)?
                    .ok_or_else(|| ApiError::InvalidUser)?
                    .discord_id
                    .ok_or_else(|| ApiError::InvalidUser)?
                    .parse()
                    .unwrap();

                let discord_api = cmd.discord_api.as_ref().unwrap();

                match discord_api
                    .get_permissions_in_guild(user_id, guild_id.parse().unwrap())
                    .await
                    .map_err(|_| ApiError::GenericError("discord error".to_string()))?
                    .contains(twilight_model::guild::Permissions::ADMINISTRATOR)
                {
                    true => Ok(Permissions::ChannelMod),
                    false => Ok(Permissions::Default),
                }
            }
            _ => unimplemented!(),
        },
        None => Ok(Permissions::Default),
    }
}

pub enum ApiError {
    InvalidUser,
    DatabaseError(DatabaseError),
    RequestError(reqwest::Error),
    GenericError(String),
}

impl From<diesel::result::Error> for ApiError {
    fn from(e: diesel::result::Error) -> Self {
        Self::DatabaseError(DatabaseError::DieselError(e))
    }
}

impl From<DatabaseError> for ApiError {
    fn from(e: DatabaseError) -> Self {
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
