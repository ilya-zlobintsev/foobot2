use crate::{database::Database, platform::ChannelIdentifier};
use anyhow::{anyhow, Context};
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
    ) -> anyhow::Result<()> {
        match channel {
            ChannelIdentifier::TwitchChannel((channel_id, _)) => {
                let twitch_api = self.twitch_api.as_ref().context("Twitch not configured")?;

                let broadcaster = twitch_api.helix_api.get_user_by_id(&channel_id).await?;

                let chat_client_guard = twitch_api.chat_client.lock().await;
                let chat_client = chat_client_guard.as_ref().expect("Chat client missing");

                tracing::info!("Sending {} to {}", msg, broadcaster.login);

                let mut msg = msg.split_whitespace().collect::<Vec<&str>>().join(" ");
                if msg.len() > 300 {
                    msg.truncate(300);
                }

                chat_client.privmsg(broadcaster.login, msg).await?;

                Ok(())
            }
            ChannelIdentifier::IrcChannel(channel) => {
                let sender = self.irc_sender.as_ref().context("IRC not configured")?;

                sender.send_privmsg(channel, &msg)?;

                Ok(())
            }
            _ => Err(anyhow!(
                "Remotely triggered commands not supported for this platform"
            )),
        }
    }
}
