pub mod connector;
pub mod discord;
pub mod irc;
pub mod local;
pub mod minecraft;
pub mod telegram;
pub mod twitch;

use crate::command_handler::CommandHandler;
use anyhow::anyhow;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::env::{self, VarError};
use std::fmt::{self, Display};
use std::hash::{Hash, Hasher};
use std::net::IpAddr;
use std::str::FromStr;

pub use connector_schema::Permissions;

#[async_trait]
pub trait ChatPlatform {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, ChatPlatformError>;

    async fn run(self);

    fn get_prefix() -> String {
        env::var("COMMAND_PREFIX").unwrap_or_else(|_| "!".to_string())
    }
}

#[async_trait]
pub trait PlatformContext {
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

    fn get_display_name(&self) -> &str;

    fn get_prefixes(&self) -> Vec<&str>;
}

#[derive(Clone)]
pub struct ServerPlatformContext {
    pub target_channel: ChannelIdentifier,
    pub executing_user: UserIdentifier,
    pub cmd: CommandHandler,
    pub display_name: String,
}

#[async_trait]
impl PlatformContext for ServerPlatformContext {
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

    fn get_display_name(&self) -> &str {
        &self.display_name
    }

    fn get_prefixes(&self) -> Vec<&str> {
        vec![""]
    }
}

#[derive(Debug)]
pub enum ChatPlatformError {
    ReqwestError(reqwest::Error),
    MissingAuthentication,
    MissingEnv(String),
    ServiceError(String),
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
    TelegramId(u64),
    IpAddr(IpAddr),
    MatrixId(String),
}

impl fmt::Display for UserIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserIdentifier::TwitchID(id) => write!(f, "twitch:{id}"),
            UserIdentifier::DiscordID(id) => write!(f, "discord:{id}"),
            UserIdentifier::TelegramId(id) => write!(f, "telegram:{id}"),
            UserIdentifier::IrcName(name) => write!(f, "irc:{name}"),
            UserIdentifier::IpAddr(addr) => write!(f, "local:{addr}"),
            UserIdentifier::MatrixId(mxid) => write!(f, "matrix:{mxid}"),
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
                .split_once(':')
                .ok_or(UserIdentifierError::MissingDelimiter)?;

            match platform {
                "twitch" => Ok(Self::TwitchID(user_id.to_owned())),
                "discord" => Ok(Self::DiscordID(user_id.to_owned())),
                "matrix" => Ok(Self::MatrixId(user_id.to_owned())),
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

// The optional values are just used for visuals, not for functionality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelIdentifier {
    TwitchChannel((String, Option<String>)), // Channel id, channel name
    DiscordChannel(String),
    IrcChannel(String),
    LocalAddress(String),
    MatrixChannel(String),
    TelegramChat((String, Option<String>)), // Chat id, chat title
    Minecraft,                              // There is a single minecraft connection
    Anonymous,                              // Used for DMs and such
}

impl ChannelIdentifier {
    pub fn new(platform: &str, id: String) -> anyhow::Result<Self> {
        match platform {
            "twitch" => Ok(Self::TwitchChannel((id, None))),
            "discord_guild" => Ok(Self::DiscordChannel(id.parse()?)),
            "local" => Ok(Self::LocalAddress(id)),
            "irc" => Ok(Self::IrcChannel(id)),
            "telegram" => Ok(Self::TelegramChat((id, None))),
            "minecraft" => Ok(Self::Minecraft),
            "matrix" => Ok(Self::MatrixChannel(id)),
            _ => Err(anyhow::anyhow!("invalid platform")),
        }
    }

    pub fn get_platform_name(&self) -> Option<&str> {
        match self {
            ChannelIdentifier::TwitchChannel(_) => Some("twitch"),
            ChannelIdentifier::DiscordChannel(_) => Some("discord_guild"),
            ChannelIdentifier::TelegramChat(_) => Some("telegram"),
            ChannelIdentifier::IrcChannel(_) => Some("irc"),
            ChannelIdentifier::LocalAddress(_) => Some("local"),
            ChannelIdentifier::Minecraft => Some("minecraft"),
            ChannelIdentifier::Anonymous => None,
            ChannelIdentifier::MatrixChannel(_) => Some("matrix"),
        }
    }

    pub fn get_channel(&self) -> Option<&str> {
        match self {
            ChannelIdentifier::TwitchChannel((id, _)) => Some(id),
            ChannelIdentifier::DiscordChannel(guild_id) => Some(guild_id),
            ChannelIdentifier::TelegramChat((id, _)) => Some(id),
            ChannelIdentifier::IrcChannel(channel) => Some(channel),
            ChannelIdentifier::LocalAddress(addr) => Some(addr),
            ChannelIdentifier::Minecraft => None,
            ChannelIdentifier::Anonymous => None,
            ChannelIdentifier::MatrixChannel(id) => Some(id),
        }
    }

    pub fn get_display_name(&self) -> Option<&str> {
        match self {
            ChannelIdentifier::TwitchChannel((_, title)) => title.as_deref(),
            ChannelIdentifier::TelegramChat((_, title)) => title.as_deref(),
            _ => None,
        }
    }
}

impl Display for ChannelIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}-{}",
            self.get_platform_name().unwrap_or("generic"),
            match self.get_display_name() {
                Some(name) => name,
                None => self.get_channel().unwrap_or("anonymous"),
            }
        )
    }
}

impl FromStr for ChannelIdentifier {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let mut split = s.split(':');
        let platform = split
            .next()
            .ok_or_else(|| anyhow!("Platform not specified!"))?;
        let id = split.next().unwrap_or_default();

        ChannelIdentifier::new(platform, id.to_string())
    }
}

impl PartialEq for ChannelIdentifier {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::TwitchChannel((l0, _)), Self::TwitchChannel((r0, _))) => l0 == r0,
            (Self::DiscordChannel(l0), Self::DiscordChannel(r0)) => l0 == r0,
            (Self::IrcChannel(l0), Self::IrcChannel(r0)) => l0 == r0,
            (Self::LocalAddress(l0), Self::LocalAddress(r0)) => l0 == r0,
            (Self::TelegramChat((l0, _)), Self::TelegramChat((r0, _))) => l0 == r0,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}
impl Eq for ChannelIdentifier {}

impl Hash for ChannelIdentifier {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            ChannelIdentifier::TwitchChannel((id, _))
            | ChannelIdentifier::TelegramChat((id, _))
            | ChannelIdentifier::DiscordChannel(id) => id.hash(state),
            ChannelIdentifier::IrcChannel(channel) => channel.hash(state),
            ChannelIdentifier::LocalAddress(addr) => addr.hash(state),
            ChannelIdentifier::Minecraft => (),
            ChannelIdentifier::Anonymous => (),
            ChannelIdentifier::MatrixChannel(id) => id.hash(state),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::platform::ChannelIdentifier;

    #[test]
    fn channel_identifier_eq() {
        assert_eq!(
            ChannelIdentifier::TelegramChat((String::from("1234"), Some(String::from("hello")))),
            ChannelIdentifier::TelegramChat((String::from("1234"), None))
        );

        assert_eq!(
            ChannelIdentifier::TwitchChannel((String::from("1234"), Some(String::from("hello")))),
            ChannelIdentifier::TwitchChannel((String::from("1234"), None))
        );
    }
}
