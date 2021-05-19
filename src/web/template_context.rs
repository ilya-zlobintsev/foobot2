use rocket::http::CookieJar;
use serde::Serialize;

use crate::database::{Database, models::{Channel, Command, User}};

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
}

#[derive(Serialize)]
pub struct LayoutContext {
    pub name: &'static str,
    pub auth_info: Option<AuthInfo>,
}

impl LayoutContext {
    pub fn new(db: &Database, cookie_jar: &CookieJar) -> Self {
        Self::new_with_auth(AuthInfo::new(db, cookie_jar))
    }

    pub fn new_with_auth(auth_info: Option<AuthInfo>) -> Self {
        Self {
            name: "layout",
            auth_info,
        }
    }
}

#[derive(Serialize)]
pub struct AuthInfo {
    pub username: String,
    pub user_id: u64,
}

impl AuthInfo {
    pub fn new(db: &Database, cookie_jar: &CookieJar) -> Option<Self> {
        match cookie_jar.get_private("session_id") {
            Some(session_cookie) => match db
                .get_web_session(session_cookie.value())
                .expect("DB Error")
            {
                Some(session) => Some(AuthInfo {
                    username: session.username,
                    user_id: session.user_id,
                }),
                None => None, // Invalid session ID
            },
            None => None,
        }
    }
}
