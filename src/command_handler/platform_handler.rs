use twitch_irc::login::RefreshingLoginCredentials;

use crate::{database::Database, platform::ChannelIdentifier};

use super::discord_api::DiscordApi;

use anyhow::anyhow;

type TwitchApi = super::twitch_api::TwitchApi<RefreshingLoginCredentials<Database>>;

#[derive(Clone, Debug)]
pub struct PlatformHandler {
    pub twitch_api: Option<TwitchApi>,
    pub discord_api: Option<DiscordApi>,
}

impl PlatformHandler {
    pub async fn send_to_channel(
        &self,
        channel: ChannelIdentifier,
        msg: String,
    ) -> anyhow::Result<()> {
        match channel {
            ChannelIdentifier::TwitchChannelID(channel_id) => {
                let twitch_api = self.twitch_api.as_ref().unwrap();

                let broadcaster = twitch_api.helix_api.get_user_by_id(&channel_id).await?;

                let chat_client_guard = twitch_api.chat_client.lock().await;

                let chat_client = chat_client_guard.as_ref().expect("Chat client missing");

                tracing::info!("Sending {} to {}", msg, broadcaster.login);

                chat_client.privmsg(broadcaster.login, msg).await?;

                Ok(())
            }
            _ => Err(anyhow!(
                "Remotely triggered commands not supported for this platform"
            )),
        }
    }
}
