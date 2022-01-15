pub mod eventsub;
pub mod model;

use anyhow::anyhow;
use futures::future::join_all;
use http::Method;
use reqwest::RequestBuilder;
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use reqwest::{header::HeaderMap, Client};
use tokio::task;

use model::*;
use serde_json::Value;
use twitch_irc::login::{LoginCredentials, RefreshingLoginCredentials, StaticLoginCredentials};

use crate::database::Database;
use crate::web::response_ok;

use self::eventsub::{EventSubSubscription, EventSubSubscriptionType};

const HELIX_URL: &'static str = "https://api.twitch.tv/helix";

#[derive(Clone, Debug)]
pub struct TwitchApi<C: LoginCredentials + Clone> {
    pub credentials: C,
    client: Client,
    moderators_cache: Arc<RwLock<HashMap<String, Vec<String>>>>,
    users_cache: Arc<RwLock<Vec<User>>>,
    app_access_token: Arc<String>,
    headers: HeaderMap,
}

impl TwitchApi<RefreshingLoginCredentials<Database>> {
    pub async fn init_refreshing(db: Database) -> anyhow::Result<Self> {
        let client_id = env::var("TWITCH_CLIENT_ID")?;
        let client_secret = env::var("TWITCH_CLIENT_SECRET")?;

        let credentials = RefreshingLoginCredentials::init(client_id, client_secret, db);

        Self::init(credentials).await
    }
}

impl TwitchApi<StaticLoginCredentials> {
    pub async fn init_with_token(access_token: &str) -> anyhow::Result<Self> {
        Self::init(StaticLoginCredentials::new(
            String::new(),
            Some(access_token.to_owned()),
        ))
        .await
    }
}

impl<C: LoginCredentials + Clone> TwitchApi<C> {
    pub async fn init(credentials: C) -> anyhow::Result<Self> {
        let mut headers = HeaderMap::new();

        let client_id = get_client_id().expect("Client ID missing");

        headers.insert("Client-Id", client_id.parse().unwrap());

        let moderators_cache = Arc::new(RwLock::new(HashMap::new()));

        let users_cache = Arc::new(RwLock::new(Vec::new()));

        let app_access_token = Self::get_app_token(
            &client_id,
            &get_client_secret().expect("client secret missing"),
        )
        .await?;

        let twitch_api = TwitchApi {
            credentials,
            client: Client::new(),
            moderators_cache,
            users_cache,
            app_access_token: Arc::new(app_access_token),
            headers,
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
        let users_cache = self.users_cache.clone();

        task::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(600)).await;

                tracing::info!("Clearing moderators cache");

                let mut moderators_cache = moderators_cache.write().expect("Failed to lock");

                moderators_cache.clear();
            }
        });

        task::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(3600)).await;

                tracing::info!("Clearing users cache");

                let mut users_cache = users_cache.write().expect("Failed to lock");

                users_cache.clear();
            }
        });

        task::spawn(async move {
            let client = Client::new();

            let user_id = env::var("SUPINIC_USER_ID").unwrap_or_default();
            let pass = env::var("SUPINIC_PASSWORD").unwrap_or_default();

            loop {
                tracing::info!("Pinging Supinic API");

                match client
                    .put("https://supinic.com/api/bot-program/bot/active")
                    .header("Authorization", format!("Basic {}:{}", user_id, pass))
                    .send()
                    .await
                {
                    Ok(response) => {
                        if !response.status().is_success() {
                            tracing::info!("Supinic API error: {:?}", response.text().await);
                        }
                    }
                    Err(e) => tracing::warn!("Failed to ping Supinic API! {:?}", e),
                }

                tokio::time::sleep(Duration::from_secs(3600)).await;
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

    pub async fn get_users(
        &self,
        logins: Option<&Vec<&str>>,
        ids: Option<&Vec<&str>>,
    ) -> anyhow::Result<Vec<User>> {
        let mut results = Vec::new();

        let mut params: Vec<(&str, &str)> = Vec::new();

        {
            let users_cache = self.users_cache.read().unwrap();

            if let Some(logins) = logins {
                for login in logins {
                    if let Some(user) = users_cache.iter().find(|user| &user.login == *login) {
                        tracing::info!("Using cache for user {}", user.login);
                        results.push(user.clone());
                    } else {
                        params.push(("login", login));
                    }
                }
            }
            if let Some(ids) = ids {
                for id in ids {
                    if let Some(user) = users_cache.iter().find(|user| &user.id == *id) {
                        tracing::info!("Using cache for user {}", user.login);
                        results.push(user.clone());
                    } else {
                        params.push(("id", id));
                    }
                }
            }
        }

        if !params.is_empty() || (logins.is_none() && ids.is_none()) {
            let response = self
                .client
                .get("https://api.twitch.tv/helix/users")
                .headers(self.headers.clone())
                .bearer_auth(self.get_token().await?)
                .query(&params)
                .send()
                .await?;

            tracing::info!("GET {}: {}", response.url(), response.status());

            let status = response.status();

            match status.is_success() {
                true => {
                    let api_results = response.json::<UsersResponse>().await?.data;

                    if api_results.len() != 0 {
                        let mut users_cache = self.users_cache.write().unwrap();

                        users_cache.extend(api_results.clone());
                    }

                    results.extend(api_results);

                    Ok(results)
                }
                false => Err(anyhow!("Response code {}", status)),
            }
        } else {
            Ok(results)
        }
    }

    async fn get_token(&self) -> anyhow::Result<String> {
        Ok(self
            .credentials
            .get_credentials()
            .await
            .map_err(|e| anyhow!("Unable to get credentials: {:?}", e))?
            .token
            .ok_or_else(|| anyhow!("Token missing"))?)
    }

    pub async fn get_self_user(&self) -> anyhow::Result<User> {
        Ok(self
            .get_users(None, None)
            .await?
            .into_iter()
            .next()
            .unwrap())
    }

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
                "https://api.ivr.fi/twitch/modsvips/{}",
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
