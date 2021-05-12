use super::schema::*;
use serde::{Serialize, Deserialize};

#[derive(Queryable, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub twitch_id: Option<String>,
    pub discord_id: Option<String>,
}

#[derive(Insertable, Default)]
#[table_name="users"]
pub struct NewUser<'a> {
    pub twitch_id: Option<&'a str>,
    pub discord_id: Option<&'a str>,
}

#[derive(Queryable, Debug, PartialEq, Eq, Serialize)]
pub struct Channel {
    pub id: u64,
    pub platform: String,
    pub channel: String,
}

#[derive(Insertable)]
#[table_name="channels"]
pub struct NewChannel<'a> {
    pub platform: &'a str,
    pub channel: &'a str,
}

#[derive(Queryable, Debug, PartialEq, Eq, Serialize)]
pub struct Command {
    pub name: String,
    pub action: String,
    pub permissions: Option<String>,
    pub channel_id: u64, 
}

#[derive(Insertable, Debug, PartialEq, Eq)]
#[table_name="commands"]
pub struct NewCommand<'a> {
    pub name: &'a str,
    pub action: &'a str,
    pub permissions: Option<&'a str>,
    pub channel_id: u64, 
}

#[derive(Queryable, Debug, PartialEq, Eq)]
pub struct UserData {
    pub name: String,
    pub value: String,
    pub public: bool,
    user_id: u64,
}

#[derive(Queryable, Insertable)]
#[table_name="web_sessions"]
pub struct WebSession {
    pub session_id: String,
    pub user_id: u64,
}