pub mod discord;
pub mod twitch;

use std::{
    env::{self, VarError},
    fmt,
};

use crate::command_handler::{twitch_api::TwitchApi, CommandHandler};
use async_trait::async_trait;
use tokio::task::JoinHandle;

use serenity::prelude::SerenityError;

use serde::{Deserialize, Serialize};

#[async_trait]
pub trait ChatPlatform {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, ChatPlatformError>;

    async fn run(self) -> JoinHandle<()>;

    fn get_prefix() -> String {
        env::var("COMMAND_PREFIX").unwrap_or_else(|_| "!".to_string())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExecutionContext {
    pub channel: ChannelIdentifier,
    pub permissions: Permissions,
}

#[derive(Debug)]
pub enum ChatPlatformError {
    ReqwestError(reqwest::Error),
    MissingAuthentication,
    DiscordError(SerenityError),
}

impl From<reqwest::Error> for ChatPlatformError {
    fn from(e: reqwest::Error) -> Self {
        ChatPlatformError::ReqwestError(e)
    }
}

impl From<VarError> for ChatPlatformError {
    fn from(_e: VarError) -> Self {
        ChatPlatformError::MissingAuthentication
    }
}

impl From<SerenityError> for ChatPlatformError {
    fn from(e: SerenityError) -> Self {
        ChatPlatformError::DiscordError(e)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserIdentifier {
    TwitchID(String),
    DiscordID(String),
}

impl fmt::Display for UserIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserIdentifier::TwitchID(id) => f.write_str(id),
            UserIdentifier::DiscordID(id) => f.write_str(id),
        }
    }
}

impl UserIdentifier {
    pub async fn from_string(
        s: &str,
        twitch_api: Option<&TwitchApi>,
    ) -> Result<Self, UserIdentifierError> {
        tracing::info!("parsing user identifier {}", s);

        if let Some(discord_user_id) = s.strip_prefix("<@!") {
            let discord_user_id = discord_user_id.strip_suffix(">").unwrap();

            Ok(UserIdentifier::DiscordID(discord_user_id.to_owned()))
        } else {
            let (platform, user_id) = s
                .split_once(":")
                .ok_or_else(|| UserIdentifierError::MissingDelimiter)?;

            match platform {
                "twitch" => match twitch_api {
                    Some(twitch_api) => Ok(Self::TwitchID(
                        twitch_api
                            .get_users(Some(&vec![user_id]), None)
                            .await
                            .expect("Twitch API Error") // TODO
                            .first()
                            .ok_or_else(|| UserIdentifierError::InvalidUser)?
                            .id
                            .clone(),
                    )),
                    None => Ok(Self::TwitchID(user_id.to_owned())),
                },
                "discord" => Ok(Self::DiscordID(user_id.to_owned())),
                _ => Err(UserIdentifierError::InvalidPlatform),
            }
        }
    }
}

#[derive(Debug)]
pub enum UserIdentifierError {
    MissingDelimiter,
    InvalidPlatform,
    InvalidUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelIdentifier {
    TwitchChannelName(String),
    DiscordGuildID(u64),
    DiscordChannelID(u64), // Mainly for DMs
}

impl ChannelIdentifier {
    pub fn new(platform: &str, id: String) -> anyhow::Result<Self> {
        match platform {
            "twitch" => Ok(Self::TwitchChannelName(id)),
            "discord_guild" => Ok(Self::DiscordGuildID(id.parse()?)),
            "discord_channel" => Ok(Self::DiscordChannelID(id.parse()?)),
            _ => Err(anyhow::anyhow!("invalid platform")),
        }
    }

    pub fn get_platform_name(&self) -> &str {
        match self {
            ChannelIdentifier::TwitchChannelName(_) => "twitch",
            ChannelIdentifier::DiscordGuildID(_) => "discord_guild",
            ChannelIdentifier::DiscordChannelID(_) => "discord_channel",
        }
    }

    pub fn get_channel(&self) -> String {
        match self {
            ChannelIdentifier::TwitchChannelName(name) => name.to_string(),
            ChannelIdentifier::DiscordGuildID(id) => id.to_string(),
            ChannelIdentifier::DiscordChannelID(id) => id.to_string(),
        }
    }
}

#[derive(Debug, Eq, Serialize, Deserialize, Clone)]
pub enum Permissions {
    Default,
    ChannelMod,
    Admin,
}

impl Permissions {
    pub fn to_string(&self) -> String {
        match self {
            Permissions::Default => "default".to_string(),
            Permissions::ChannelMod => "channel_mod".to_string(),
            Permissions::Admin => "admin".to_string(),
        }
    }
}

impl PartialEq for Permissions {
    fn eq(&self, other: &Self) -> bool {
        self == other
    }
}

/*impl PartialOrd for Permissions {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self {
            Permissions::BotAdmin => match other {
                Permissions::BotAdmin => Some(Ordering::Equal),
                Permissions::ChannelMod => Some(Ordering::Greater),
            }
            Permissions::ChannelMod => match other {
                Permissions::BotAdmin => Some(Ordering::Less),
                Permissions::ChannelMod => Some(Ordering::Equal),
            }
        }
    }
}*/
