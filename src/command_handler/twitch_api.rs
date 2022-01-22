pub mod eventsub;
pub mod helix;
pub mod model;

use futures::future::join_all;
use http::Method;
use reqwest::RequestBuilder;
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::Mutex;

use reqwest::{header::HeaderMap, Client};
use tokio::task;

use model::*;
use serde_json::Value;
use twitch_irc::login::{LoginCredentials, RefreshingLoginCredentials};

use crate::database::Database;
use crate::platform::twitch;
use crate::web::response_ok;

use self::eventsub::{EventSubSubscription, EventSubSubscriptionType};
use self::helix::HelixApi;

const HELIX_URL: &'static str = "https://api.twitch.tv/helix";

#[derive(Clone, Debug)]
pub struct TwitchApi<C: LoginCredentials + Clone> {
    pub helix_api: HelixApi<C>,
    pub chat_client: Arc<Mutex<Option<twitch::TwitchClient>>>,
    moderators_cache: Arc<RwLock<HashMap<String, Vec<String>>>>,
    client: Client,
    headers: HeaderMap,
    app_access_token: Arc<String>,
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
        let mut headers = HeaderMap::new();

        let client_id = get_client_id().expect("Client ID missing");

        headers.insert("Client-Id", client_id.parse().unwrap());

        let app_access_token = Self::get_app_token(
            &client_id,
            &get_client_secret().expect("client secret missing"),
        )
        .await?;

        let twitch_api = TwitchApi {
            helix_api: HelixApi::with_credentials(credentials).await,
            client: Client::new(),
            chat_client: Arc::new(Mutex::new(None)),
            headers,
            app_access_token: Arc::new(app_access_token),
            moderators_cache: Arc::new(RwLock::new(HashMap::new())),
        };

        twitch_api.start_cron().await;

        let mut request_handles = Vec::new();

        for subscription in twitch_api.get_eventsub_subscriptions().await? {
            let twitch_api = twitch_api.clone();

            request_handles.push(task::spawn(async move {
                tracing::info!("Removing old subscription {}", subscription.sub_type);

                twitch_api
                    .delete_eventsub_subscription(&subscription.id)
                    .await
                    .expect("Failed to remove EventSub subscription");
            }));
        }

        join_all(request_handles).await;

        Ok(twitch_api)
    }

    // TODO
    pub async fn get_app_token(
        client_id: &str,
        client_secret: &str,
    ) -> Result<String, reqwest::Error> {
        let client = Client::new();

        let response: serde_json::Value = client.post("https://id.twitch.tv/oauth2/token")
            .query(&[("client_id", client_id), ("client_secret", client_secret), ("grant_type", "client_credentials"), ("scope", "moderation:read channel:edit:commercial channel:manage:broadcast channel:moderate chat:edit")])
            .send().await?.json().await?;

        // tracing::info!("{:?}", response);

        Ok(response
            .get("access_token")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string())
    }

    pub async fn start_cron(&self) {
        let moderators_cache = self.moderators_cache.clone();

        task::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(600)).await;

                tracing::info!("Clearing moderators cache");

                let mut moderators_cache = moderators_cache.write().expect("Failed to lock");

                moderators_cache.clear();
            }
        });
    }

    pub async fn validate_oauth(oauth: &str) -> Result<ValidationResponse, reqwest::Error> {
        let client = Client::new();
        let response = client
            .get("https://id.twitch.tv/oauth2/validate")
            .header("Authorization", format!("Bearer {}", oauth))
            .send()
            .await?;
        // tracing::info!("Validating twitch API token: {}", response.status());
        Ok(response.json().await?)
    }

    /*pub fn get_client_id(&self) -> &str {
        self.headers.get("Client-Id").unwrap().to_str().unwrap()
    }*/

    pub async fn get_channel_mods(
        &self,
        channel_login: &str,
    ) -> Result<Vec<String>, reqwest::Error> {
        // This is not very idiomatic but i couldnt figure out how to make it work otherwise
        {
            let moderators_cache = self.moderators_cache.read().unwrap();

            if let Some(mods) = moderators_cache.get(channel_login) {
                return Ok(mods.clone());
            }
        }

        let response = self
            .client
            .get(format!(
                "https://api.ivr.fi/v2/twitch/modvip/{}",
                channel_login
            ))
            .send()
            .await?;

        tracing::info!("GET {}: {}", response.url(), response.status());

        let lookup = response.json::<IvrModInfo>().await?;

        let mut mods = vec![channel_login.to_owned()];

        for moderator in lookup.mods {
            mods.push(moderator.login);
        }

        tracing::debug!("{:?}", mods);

        // Err(_) => self.get_channel_mods_from_irc(channel_login).await?,

        let mut moderators_cache = self.moderators_cache.write().unwrap();

        moderators_cache.insert(channel_login.to_string(), mods.clone());

        Ok(mods)
    }
    // This terrible abomination has to exist because twitch doesn't provide an endpoint for this that doesn't require channel auth
    // /// Returns the list of logins of channel moderators. Don't expect this to be efficient
    /*async fn get_channel_mods_from_irc(
        &self,
        channel_login: &str,
    ) -> Result<Vec<String>, reqwest::Error> {
        let oauth = self.get_oauth();

        let login = Self::validate_oauth(oauth).await?.login;

        let config =
            ClientConfig::new_simple(StaticLoginCredentials::new(login, Some(oauth.to_owned())));

        let (mut incoming_messages, client) =
            TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(config);

        client.join(channel_login.to_owned());

        client
            .privmsg(channel_login.to_owned(), "/mods".to_owned())
            .await
            .expect("Failed to send");

        let mut mods = vec![channel_login.to_owned()];

        while let Some(msg) = incoming_messages.recv().await {
            match msg {
                ServerMessage::Notice(notice) => {
                    if let Some(mods_list) = notice
                        .message_text
                        .strip_prefix("The moderators of this channel are:")
                    {
                        mods.append(
                            &mut mods_list
                                .trim()
                                .split(", ")
                                .map(|s| s.to_string())
                                .collect(),
                        );
                        break;
                    }
                }
                _ => {}
            }
        }

        Ok(mods)
    }*/

    fn app_request(&self, method: Method, path: &str) -> RequestBuilder {
        self.client
            .request(method, format!("{}{}", HELIX_URL, path))
            .headers(self.headers.clone())
            .bearer_auth(&self.app_access_token)
    }

    fn app_get(&self, path: &str) -> RequestBuilder {
        self.app_request(Method::GET, path)
    }

    fn app_post(&self, path: &str) -> RequestBuilder {
        self.app_request(Method::POST, path)
    }

    fn app_delete(&self, path: &str) -> RequestBuilder {
        self.app_request(Method::DELETE, path)
    }

    pub async fn add_eventsub_subscription(
        &self,
        subscription: EventSubSubscriptionType,
    ) -> anyhow::Result<()> {
        let response = self
            .app_post("/eventsub/subscriptions")
            .json(&subscription.build_body())
            .send()
            .await?;

        response_ok(&response)?;

        tracing::info!("Succesfully added EventSub subscription {:?}", subscription);

        Ok(())
    }

    pub async fn get_eventsub_subscriptions(&self) -> anyhow::Result<Vec<EventSubSubscription>> {
        let response = self.app_get("/eventsub/subscriptions").send().await?;

        let mut v: Value = response.json().await?;

        let data = v["data"].take();

        Ok(serde_json::from_value(data)?)
    }

    pub async fn delete_eventsub_subscription(&self, id: &str) -> anyhow::Result<()> {
        response_ok(
            &self
                .app_delete("/eventsub/subscriptions")
                .query(&[("id", id)])
                .send()
                .await?,
        )
    }
}

pub fn get_client_id() -> Option<String> {
    env::var("TWITCH_CLIENT_ID").ok()
}

pub fn get_client_secret() -> Option<String> {
    env::var("TWITCH_CLIENT_SECRET").ok()
}
