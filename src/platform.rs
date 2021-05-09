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
    
    async fn run(self) -> ();
    
    fn get_prefix() -> String {
        env::var("COMMAND_PREFIX").unwrap_or_else(|_| "!".to_string())
    }
    
}

pub struct ExecutionContext {
    channel: ChannelIdentifier
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

pub enum UserIdentifier {
    TwitchID(String),
    DiscordID(String),
}

pub enum ChannelIdentifier {
    TwitchChannelID(String),
    DiscordGuildID(String),
}