pub mod discord;
pub mod twitch;

use crate::command_handler::CommandHandler;

use async_trait::async_trait;
use tokio::task::JoinHandle;

use serde::{Deserialize, Serialize};

use std::env::{self, VarError};
use std::fmt;

#[async_trait]
pub trait ChatPlatform {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, ChatPlatformError>;

    async fn run(self) -> JoinHandle<()>;

    fn get_prefix() -> String {
        env::var("COMMAND_PREFIX").unwrap_or_else(|_| "!".to_string())
    }
}

#[async_trait]
pub trait ExecutionContext {
    async fn get_permissions_internal(&self) -> Permissions;

    async fn get_permissions(&self) -> Permissions {
        if let Ok(admin_user) = env::var("ADMIN_USER") {
            if admin_user == self.get_user_identifier().to_string() {
                return Permissions::Admin;
            }
        }

        self.get_permissions_internal().await
    }

    fn get_channel(&self) -> ChannelIdentifier;

    fn get_user_identifier(&self) -> UserIdentifier;
}

#[derive(Debug)]
pub enum ChatPlatformError {
    ReqwestError(reqwest::Error),
    MissingAuthentication,
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserIdentifier {
    TwitchID(String),
    DiscordID(String),
}

impl fmt::Display for UserIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserIdentifier::TwitchID(id) => f.write_str(&format!("twitch:{}", id)),
            UserIdentifier::DiscordID(id) => f.write_str(&format!("discord:{}", id)),
        }
    }
}

impl UserIdentifier {
    pub fn from_string(s: &str) -> Result<Self, UserIdentifierError> {
        tracing::info!("parsing user identifier {}", s);

        if let Some(discord_user_id) = s.strip_prefix("<@!") {
            let discord_user_id = discord_user_id.strip_suffix(">").unwrap();

            Ok(UserIdentifier::DiscordID(discord_user_id.to_owned()))
        } else {
            let (platform, user_id) = s
                .split_once(":")
                .ok_or_else(|| UserIdentifierError::MissingDelimiter)?;

            match platform {
                "twitch" => Ok(Self::TwitchID(user_id.to_owned())),
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelIdentifier {
    TwitchChannelName(String),
    DiscordGuildID(String),
    Anonymous, // Used for DMs and such
}

impl ChannelIdentifier {
    pub fn new(platform: &str, id: String) -> anyhow::Result<Self> {
        match platform {
            "twitch" => Ok(Self::TwitchChannelName(id)),
            "discord_guild" => Ok(Self::DiscordGuildID(id.parse()?)),
            _ => Err(anyhow::anyhow!("invalid platform")),
        }
    }

    pub fn get_platform_name(&self) -> Option<&str> {
        match self {
            ChannelIdentifier::TwitchChannelName(_) => Some("twitch"),
            ChannelIdentifier::DiscordGuildID(_) => Some("discord_guild"),
            ChannelIdentifier::Anonymous => None,
        }
    }

    pub fn get_channel(&self) -> Option<&str> {
        match self {
            ChannelIdentifier::TwitchChannelName(name) => Some(&name),
            ChannelIdentifier::DiscordGuildID(id) => Some(&id),
            ChannelIdentifier::Anonymous => None,
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
