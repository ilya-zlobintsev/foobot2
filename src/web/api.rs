use rocket::response::Responder;
use rocket::response::{self};
use rocket::{
    get,
    http::{ContentType, CookieJar, Status},
    Response, State,
};

use crate::{
    command_handler::twitch_api::TwitchApi, database::Database, platform::ChannelIdentifier,
};

use super::template_context::AuthInfo;

// TODO: Error handling
#[get("/permissions?<channel_id>")]
pub async fn get_permissions(
    channel_id: &str,
    jar: &CookieJar<'_>,
    db: &State<Database>,
    twitch_api: &State<TwitchApi>,
) -> Result<&'static str, ApiError> {
    match AuthInfo::new(db, jar) {
        Some(auth_info) => match db.get_channel_by_id(channel_id.parse().expect("Invalid ID"))? {
            Some(channel) => match ChannelIdentifier::new(&channel.platform, channel.channel)? {
                ChannelIdentifier::TwitchChannelName(twitch_channel) => {
                    let twitch_id = db
                        .get_user_by_id(auth_info.user_id)?
                        .ok_or_else(|| ApiError::GenericError("Invalid user".to_string()))?
                        .twitch_id
                        .ok_or_else(|| {
                            ApiError::GenericError("No registered on this platform".to_string())
                        })?;

                    match twitch_api
                        .get_channel_mods(&twitch_channel)
                        .await?
                        .contains(
                            &twitch_api
                                .get_users(None, Some(&vec![&twitch_id]))
                                .await?
                                .data
                                .first()
                                .unwrap()
                                .display_name,
                        ) {
                        true => Ok("channel_mod"),
                        false => Ok("none"),
                    }
                }
                _ => unimplemented!(),
            },
            None => Ok("none"),
        },
        None => Ok("none"),
    }
}

pub enum ApiError {
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
