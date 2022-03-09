use crate::{
    database::Database,
    platform::{twitch, ChannelIdentifier},
};
use anyhow::Error;
use std::fmt::Display;
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

                let chat_sender_guard = twitch_api.chat_sender.lock().await;
                let chat_sender = chat_sender_guard.as_ref().expect("Chat client missing");

                tracing::info!("Sending {} to {}", msg, broadcaster.login);

                let message = msg.split_whitespace().collect::<Vec<&str>>().join(" ");

                chat_sender
                    .send(twitch::SenderMessage::Privmsg(twitch::Privmsg {
                        channel_login: broadcaster.login,
                        message,
                        reply_to_id: None,
                    }))
                    .unwrap();

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
