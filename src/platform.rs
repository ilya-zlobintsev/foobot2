pub mod discord;
pub mod irc;
pub mod twitch;

use crate::command_handler::CommandHandler;

use async_trait::async_trait;

use serde::{Deserialize, Serialize};

use std::cmp::Ordering;
use std::env::{self, VarError};
use std::fmt::{self, Display};

#[async_trait]
pub trait ChatPlatform {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, ChatPlatformError>;

    async fn run(self);

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

#[derive(Clone)]
pub struct ServerExecutionContext {
    pub target_channel: ChannelIdentifier,
    pub executing_user: UserIdentifier,
    pub cmd: CommandHandler,
}

#[async_trait]
impl ExecutionContext for ServerExecutionContext {
    async fn get_permissions_internal(&self) -> Permissions {
        let user = self
            .cmd
            .db
            .get_user(&self.executing_user)
            .expect("DB error")
            .expect("Invalid user");

        self.cmd
            .get_permissions_in_channel(user, &self.target_channel)
            .await
            .expect("Failed to get permissions")
    }

    fn get_channel(&self) -> ChannelIdentifier {
        self.target_channel.clone()
    }

    fn get_user_identifier(&self) -> UserIdentifier {
        self.executing_user.clone()
    }
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum UserIdentifier {
    TwitchID(String),
    DiscordID(String),
    IrcName(String),
}

impl fmt::Display for UserIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserIdentifier::TwitchID(id) => f.write_str(&format!("twitch:{}", id)),
            UserIdentifier::DiscordID(id) => f.write_str(&format!("discord:{}", id)),
            UserIdentifier::IrcName(name) => f.write_str(&format!("irc:{}", name)),
        }
    }
}

impl UserIdentifier {
    pub fn from_string(s: &str) -> Result<Self, UserIdentifierError> {
        tracing::info!("parsing user identifier {}", s);

        if let Some(discord_user_id) = s.strip_prefix("<@!") {
            let discord_user_id = discord_user_id.strip_suffix('>').unwrap();

            Ok(UserIdentifier::DiscordID(discord_user_id.to_owned()))
        } else {
            let (platform, user_id) = s
                .split_once(":")
                .ok_or(UserIdentifierError::MissingDelimiter)?;

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
    TwitchChannelID(String),
    DiscordGuildID(String),
    IrcChannel(String),
    Anonymous, // Used for DMs and such
}

impl ChannelIdentifier {
    pub fn new(platform: &str, id: String) -> anyhow::Result<Self> {
        match platform {
            "twitch" => Ok(Self::TwitchChannelID(id)),
            "discord_guild" => Ok(Self::DiscordGuildID(id.parse()?)),
            _ => Err(anyhow::anyhow!("invalid platform")),
        }
    }

    pub fn get_platform_name(&self) -> Option<&str> {
        match self {
            ChannelIdentifier::TwitchChannelID(_) => Some("twitch"),
            ChannelIdentifier::DiscordGuildID(_) => Some("discord_guild"),
            ChannelIdentifier::IrcChannel(_) => Some("irc"),
            ChannelIdentifier::Anonymous => None,
        }
    }

    pub fn get_channel(&self) -> Option<&str> {
        match self {
            ChannelIdentifier::TwitchChannelID(id) => Some(id),
            ChannelIdentifier::DiscordGuildID(id) => Some(id),
            ChannelIdentifier::IrcChannel(channel) => Some(channel),
            ChannelIdentifier::Anonymous => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum Permissions {
    Default,
    ChannelMod,
    Admin,
}

impl Display for Permissions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Permissions::Default => "default",
                Permissions::ChannelMod => "channel_mod",
                Permissions::Admin => "admin",
            }
        )
    }
}

impl PartialOrd for Permissions {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self {
            Permissions::Admin => match other {
                Permissions::Admin => Some(Ordering::Equal),
                Permissions::ChannelMod | Permissions::Default => Some(Ordering::Greater),
            },
            Permissions::ChannelMod => match other {
                Permissions::Admin => Some(Ordering::Less),
                Permissions::ChannelMod => Some(Ordering::Equal),
                Permissions::Default => Some(Ordering::Greater),
            },
            Permissions::Default => match other {
                Permissions::ChannelMod | Permissions::Admin => Some(Ordering::Less),
                Permissions::Default => Some(Ordering::Equal),
            },
        }
    }
}
