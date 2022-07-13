pub mod eventsub;
pub mod helix;
pub mod model;

use self::helix::HelixApi;
use crate::database::Database;
use reqwest::Client;
use std::env;
use twitch_irc::login::{LoginCredentials, RefreshingLoginCredentials, StaticLoginCredentials};

const APP_SCOPES: &[&str] = &["moderation:read", "channel:moderate", "chat:edit"];

#[derive(Clone, Debug)]
pub struct TwitchApi<C: LoginCredentials + Clone> {
    pub helix_api: HelixApi<C>,
    pub helix_api_app: HelixApi<StaticLoginCredentials>,
}

impl TwitchApi<RefreshingLoginCredentials<Database>> {
    pub async fn init_refreshing(db: Database) -> anyhow::Result<Self> {
        let client_id = env::var("TWITCH_CLIENT_ID")?;
        let client_secret = env::var("TWITCH_CLIENT_SECRET")?;

        let credentials = RefreshingLoginCredentials::init(client_id, client_secret, db);

        Self::init(credentials).await
    }
}

impl<C: LoginCredentials + Clone> TwitchApi<C> {
    pub async fn init(credentials: C) -> anyhow::Result<Self> {
        let client_id = get_client_id().expect("Client ID missing");

        let app_access_token = Self::get_app_token(
            &client_id,
            &get_client_secret().expect("client secret missing"),
        )
        .await?;

        let twitch_api = TwitchApi {
            helix_api: HelixApi::with_credentials(credentials).await,
            helix_api_app: HelixApi::with_credentials(StaticLoginCredentials::new(
                String::new(),
                Some(app_access_token),
            ))
            .await,
        };

        Ok(twitch_api)
    }

    // TODO
    async fn get_app_token(client_id: &str, client_secret: &str) -> Result<String, reqwest::Error> {
        let client = Client::new();

        let response: serde_json::Value = client
            .post("https://id.twitch.tv/oauth2/token")
            .query(&[
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("grant_type", "client_credentials"),
                ("scope", &APP_SCOPES.join(" ")),
            ])
            .send()
            .await?
            .json()
            .await?;

        // tracing::info!("{:?}", response);

        Ok(response
            .get("access_token")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string())
    }
}

pub fn get_client_id() -> Option<String> {
    env::var("TWITCH_CLIENT_ID").ok()
}

pub fn get_client_secret() -> Option<String> {
    env::var("TWITCH_CLIENT_SECRET").ok()
}
