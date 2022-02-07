use twitch_irc::login::RefreshingLoginCredentials;

use crate::database::Database;

use super::discord_api::DiscordApi;

type TwitchApi = super::twitch_api::TwitchApi<RefreshingLoginCredentials<Database>>;

#[derive(Clone, Debug)]
pub struct PlatformHandler {
    pub twitch_api: Option<TwitchApi>,
    pub discord_api: Option<DiscordApi>,
}

impl PlatformHandler {
    pub fn new(twitch_api: Option<TwitchApi>, discord_api: Option<DiscordApi>) -> Self {
        Self {
            twitch_api,
            discord_api,
        }
    }
}
