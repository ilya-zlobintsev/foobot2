pub mod twitch;
pub mod discord;

use std::env::{self, VarError};

use async_trait::async_trait;
use tokio::task::JoinHandle;
use crate::command_handler::CommandHandler;

use serenity::prelude::SerenityError;

#[async_trait]
pub trait ChatPlatform {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, ChatPlatformError>;
    
    async fn run(self) -> JoinHandle<()>;
    
    fn get_prefix() -> String {
        env::var("COMMAND_PREFIX").unwrap_or_else(|_| "!".to_string())
    }
    
}

#[derive(Debug)]
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

#[derive(Clone, Debug)]
pub enum UserIdentifier {
    TwitchID(String),
    DiscordID(String),
}

#[derive(Debug, Clone)]
pub enum ChannelIdentifier {
    TwitchChannelName(String),
    DiscordGuildID(String),
    DiscordChannelID(String), // Mainly for DMs
}

impl ChannelIdentifier {
    pub fn get_platform_name(&self) -> &str {
        match self {
            ChannelIdentifier::TwitchChannelName(_) => "twitch",
            ChannelIdentifier::DiscordGuildID(_) => "discord_guild",
            ChannelIdentifier::DiscordChannelID(_) => "discord_channel",
        }
    }
    
    pub fn get_channel(&self) -> &str {
        match self {
            ChannelIdentifier::TwitchChannelName(name) => name,
            ChannelIdentifier::DiscordGuildID(id) => id,
            ChannelIdentifier::DiscordChannelID(id) => id,
        }
    }
}

#[derive(Debug, Eq)]
pub enum Permissions {
    Default,
    ChannelMod,
    // BotAdmin,
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