use crate::{
    database::Database,
    platform::{twitch, ChannelIdentifier},
};
use anyhow::Error;
use std::{fmt::Display, time::Duration};
use tokio::time::sleep;
use twitch_irc::login::RefreshingLoginCredentials;

use super::discord_api::DiscordApi;
use irc::client::Sender as IrcSender;

type TwitchApi = super::twitch_api::TwitchApi<RefreshingLoginCredentials<Database>>;

#[derive(Clone, Debug)]
pub struct PlatformHandler {
    pub twitch_api: Option<TwitchApi>,
    pub discord_api: Option<DiscordApi>,
    pub irc_sender: Option<IrcSender>,
}

impl PlatformHandler {
    pub async fn send_to_channel(
        &self,
        channel: ChannelIdentifier,
        msg: String,
    ) -> Result<(), PlatformHandlerError> {
        match channel {
            ChannelIdentifier::TwitchChannel((channel_id, _)) => {
                let twitch_api = self
                    .twitch_api
                    .as_ref()
                    .ok_or(PlatformHandlerError::Unconfigured)?;

                let broadcaster = twitch_api.helix_api.get_user_by_id(&channel_id).await?;

                let chat_client_guard = twitch_api.chat_client.lock().await;
                let chat_client = chat_client_guard.as_ref().expect("Chat client missing");

                tracing::info!("Sending {} to {}", msg, broadcaster.login);

                let msg = msg.split_whitespace().collect::<Vec<&str>>().join(" ");

                if msg.len() > twitch::MSG_LENGTH_LIMIT {
                    let mut section = String::new();

                    for ch in msg.chars() {
                        section.push(ch);
                        if section.len() == twitch::MSG_LENGTH_LIMIT {
                            chat_client
                                .privmsg(broadcaster.login.clone(), section.clone())
                                .await
                                .map_err(|e| Error::new(e))?;
                            section.clear();
                            sleep(Duration::from_millis(500)).await; // scuffed rate limiting
                        }
                    }
                    if !section.is_empty() {
                        chat_client
                            .privmsg(broadcaster.login, section)
                            .await
                            .map_err(|e| Error::new(e))?;
                    }
                } else {
                    chat_client
                        .privmsg(broadcaster.login, msg)
                        .await
                        .map_err(|e| Error::new(e))?;
                }

                Ok(())
            }
            ChannelIdentifier::IrcChannel(channel) => {
                let sender = self
                    .irc_sender
                    .as_ref()
                    .ok_or(PlatformHandlerError::Unconfigured)?;

                sender
                    .send_privmsg(channel, &msg)
                    .map_err(|e| Error::new(e))?;

                Ok(())
            }
            _ => Err(PlatformHandlerError::Unsupported),
        }
    }
}

#[derive(Debug)]
pub enum PlatformHandlerError {
    Unsupported,
    Unconfigured,
    PlatformError(anyhow::Error),
}

impl From<anyhow::Error> for PlatformHandlerError {
    fn from(e: anyhow::Error) -> Self {
        PlatformHandlerError::PlatformError(e)
    }
}

impl std::error::Error for PlatformHandlerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl Display for PlatformHandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                PlatformHandlerError::Unconfigured => String::from("Platform is not configured"),
                PlatformHandlerError::Unsupported =>
                    String::from("Remote message sending is not supported for this platform"),
                PlatformHandlerError::PlatformError(e) => format!("Platform error: {}", e),
            }
        )
    }
}
