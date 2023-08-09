use std::str::FromStr;

use crate::platform::ChannelIdentifier;

use super::schema::*;
use diesel::Queryable;
use serde::{Deserialize, Serialize};
use strum::EnumString;

#[derive(Queryable, Identifiable, AsChangeset, Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: u64,
    pub twitch_id: Option<String>,
    pub discord_id: Option<String>,
    pub irc_name: Option<String>,
    pub local_addr: Option<String>,
    pub telegram_id: Option<String>,
    pub matrix_id: Option<String>,
}

impl User {
    pub fn merge(&mut self, other: User) {
        if self.twitch_id.is_none() && other.twitch_id.is_some() {
            self.twitch_id = other.twitch_id;
        }

        if self.discord_id.is_none() && other.discord_id.is_some() {
            self.discord_id = other.discord_id;
        }
    }
}

#[derive(Insertable, Default)]
#[diesel(table_name = users)]
pub struct NewUser<'a> {
    pub twitch_id: Option<&'a str>,
    pub discord_id: Option<&'a str>,
    pub irc_name: Option<&'a str>,
    pub local_addr: Option<String>,
    pub telegram_id: Option<String>,
    pub matrix_id: Option<&'a str>,
}

#[derive(Queryable, Debug, PartialEq, Eq, Serialize, Clone)]
pub struct Channel {
    pub id: u64,
    pub platform: String,
    pub channel: String,
}

impl Channel {
    pub fn get_identifier(&self) -> ChannelIdentifier {
        ChannelIdentifier::from_str(&format!("{}:{}", self.platform, self.channel)).unwrap()
    }
}

#[derive(Insertable)]
#[diesel(table_name = channels)]
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
    pub cooldown: Option<u64>,
    pub triggers: Option<String>,
    #[diesel(deserialize_as = String)]
    pub mode: CommandMode,
}

#[derive(Debug, PartialEq, Eq, Serialize, EnumString, strum::Display)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum CommandMode {
    Template,
    Hebi,
}

impl TryFrom<String> for CommandMode {
    type Error = strum::ParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}

#[derive(Insertable, Debug, PartialEq, Eq)]
#[diesel(table_name = commands)]
pub struct NewCommand<'a> {
    pub name: &'a str,
    pub action: &'a str,
    pub permissions: Option<&'a str>,
    pub channel_id: u64,
    pub cooldown: u64,
}

#[derive(Queryable, Insertable, Debug, PartialEq, Eq)]
#[diesel(table_name = user_data)]
pub struct UserData {
    pub name: String,
    pub value: String,
    pub public: bool,
    pub user_id: u64,
}

#[derive(Insertable)]
#[diesel(table_name = user_data)]
pub struct UserDataUserId {
    pub user_id: u64,
}

#[derive(Queryable, Insertable, Clone, Serialize, Debug)]
#[diesel(table_name = web_sessions)]
pub struct WebSession {
    #[serde(skip)]
    pub session_id: String,
    pub user_id: u64,
    pub username: String,
}

#[derive(Insertable)]
#[diesel(table_name = eventsub_triggers)]
pub struct NewEventSubTrigger<'a> {
    pub broadcaster_id: &'a str,
    pub event_type: &'a str,
    pub action: &'a str,
    pub creation_payload: &'a str,
    pub id: &'a str,
}

#[derive(Queryable, Serialize)]
pub struct EventSubTrigger {
    pub broadcaster_id: String,
    pub event_type: String,
    pub action: String,
    pub creation_payload: String,
    pub id: String,
    #[diesel(deserialize_as = String)]
    pub mode: CommandMode,
}

#[derive(Queryable)]
pub struct Prefix {
    pub channel_id: u64,
    pub prefix: String,
}

#[derive(Queryable, Insertable)]
#[diesel(table_name = mirror_connections)]
pub struct MirrorConnection {
    pub from_channel_id: u64,
    pub to_channel_id: u64,
}

#[derive(Queryable, Insertable, Debug, Serialize)]
#[diesel(table_name = filters)]
pub struct Filter {
    #[serde(skip)]
    pub channel_id: u64,
    pub regex: String,
    pub block_message: bool,
    pub replacement: Option<String>,
}

#[derive(Queryable, Insertable)]
#[diesel(table_name = hebi_data)]
pub struct HebiData {
    pub channel_id: u64,
    pub name: String,
    pub value: Option<String>,
}

#[derive(Queryable, Insertable)]
#[diesel(table_name = geohub_link)]
pub struct GeohubLink {
    pub user_id: u64,
    pub channel_id: u64,
    pub geohub_name: String,
}

#[cfg(test)]
mod tests {
    use crate::platform::ChannelIdentifier;

    use super::Channel;

    #[test]
    fn channel_to_identifier() {
        let channel = Channel {
            id: 1,
            platform: String::from("twitch"),
            channel: String::from("123"),
        };

        assert_eq!(
            channel.get_identifier(),
            ChannelIdentifier::TwitchChannel((String::from("123"), None))
        )
    }
}
