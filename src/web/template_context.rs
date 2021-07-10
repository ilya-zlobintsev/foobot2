use rocket::{http::Status, outcome::Outcome, request::FromRequest};
use serde::Serialize;

use crate::command_handler::CommandHandler;
use crate::database::models::{Channel, Command, User, WebSession};

#[derive(Serialize)]
pub struct IndexContext {
    pub parent_context: LayoutContext,
    pub channel_amount: i64,
}

#[derive(Serialize)]
pub struct ChannelsContext {
    pub parent_context: LayoutContext,
    pub channels: Vec<Channel>,
}

#[derive(Serialize)]
pub struct CommandsContext {
    pub parent_context: LayoutContext,
    pub channel: String,
    pub commands: Vec<Command>,
}

#[derive(Serialize)]
pub struct AuthenticateContext {
    pub parent_context: LayoutContext,
}

#[derive(Serialize)]
pub struct ProfileContext {
    pub user: User,
    pub spotify_connected: bool,
    pub parent_context: LayoutContext,
    pub lastfm_name: Option<String>,
}

#[derive(Serialize)]
pub struct LayoutContext {
    pub name: &'static str,
    pub session: Option<WebSession>,
}

impl LayoutContext {
    pub fn new_with_auth(session: Option<WebSession>) -> Self {
        Self {
            name: "layout",
            session,
        }
    }
}

#[async_trait]
impl<'r> FromRequest<'r> for WebSession {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let db = &request
            .rocket()
            .state::<CommandHandler>()
            .expect("Missing state")
            .db;

        match request.cookies().get_private("session_id") {
            Some(session_id) => match db.get_web_session(session_id.value()).expect("DB Error") {
                Some(web_session) => Outcome::Success(web_session),
                None => Outcome::Failure((Status::Unauthorized, ())),
            },
            None => Outcome::Failure((Status::Unauthorized, ())),
        }
    }
}
