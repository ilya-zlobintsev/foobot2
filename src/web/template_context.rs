use rocket::http::CookieJar;
use serde::Serialize;

use crate::database::{
    models::{Channel, Command},
    Database,
};

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
pub struct LayoutContext {
    pub name: &'static str,
    pub username: Option<String>,
}

impl LayoutContext {
    pub fn new(db: &Database, cookie_jar: &CookieJar) -> Self {
        let username = match cookie_jar.get_private("session_id") {
            Some(session_id) => match db.get_web_session(session_id.value()).expect("DB Error") {
                Some(session) => Some(session.username.clone()),
                None => None, // Invalid session ID
            },
            None => None,
        };

        Self {
            name: "layout",
            username,
        }
    }
}
