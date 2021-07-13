pub mod model;

use std::collections::HashMap;
use std::env;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use reqwest::{header::HeaderMap, Client};
use serde_json::json;
use tokio::task;

use model::*;

#[derive(Clone)]
pub struct TwitchApi {
    headers: Arc<HeaderMap>,
    client: Client,
    moderators_cache: Arc<RwLock<HashMap<String, Vec<String>>>>,
    users_cache: Arc<RwLock<Vec<User>>>,
    app_access_token: Option<Arc<String>>,
}

impl TwitchApi {
    pub async fn init(access_token: &str) -> Result<Self, reqwest::Error> {
        let oauth = match access_token.strip_prefix("oauth:") {
            Some(res) => res,
            None => access_token,
        };

        let validation = Self::validate_oauth(oauth).await?;

        let mut headers = HeaderMap::new();

        headers.insert("Client-Id", validation.client_id.parse().unwrap());
        headers.insert(
            "Authorization",
            format!("Bearer {}", oauth).parse().unwrap(),
        );
        headers.insert("Content-Type", "application/json".parse().unwrap());

        let moderators_cache = Arc::new(RwLock::new(HashMap::new()));

        let users_cache = Arc::new(RwLock::new(Vec::new()));

        let twitch_api = TwitchApi {
            headers: Arc::new(headers),
            client: Client::new(),
            moderators_cache,
            users_cache,
            app_access_token: match env::var("TWITCH_CLIENT_SECRET") {
                Ok(secret) => Some(Arc::new(
                    Self::get_app_token(&validation.client_id, &secret).await?,
                )),
                Err(_) => None,
            },
        };

        /*if let Some(_) = twitch_api.app_access_token {
            for subscription in twitch_api.list_eventsub_subscriptions().await?.data {
                twitch_api
                    .delete_eventsub_subscription(&subscription.id)
                    .await?;
            }
        }*/

        twitch_api.start_cron().await;

        Ok(twitch_api)
    }

    // TODO
    pub async fn get_app_token(
        client_id: &str,
        client_secret: &str,
    ) -> Result<String, reqwest::Error> {
        let client = Client::new();

        let response: serde_json::Value = client.post("https://id.twitch.tv/oauth2/token").query(&[("client_id", client_id), ("client_secret", client_secret), ("grant_type", "client_credentials"), ("scope", "moderation:read channel:edit:commercial channel:manage:broadcast channel:moderate chat:edit")]).send().await?.json().await?;

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

    pub fn get_oauth(&self) -> &str {
        self.headers
            .get("Authorization")
            .unwrap()
            .to_str()
            .unwrap()
            .strip_prefix("Bearer ")
            .unwrap()
    }

    pub fn get_client_id(&self) -> &str {
        self.headers.get("Client-Id").unwrap().to_str().unwrap()
    }

    pub async fn get_users(
        &self,
        logins: Option<&Vec<&str>>,
        ids: Option<&Vec<&str>>,
    ) -> Result<Vec<User>, reqwest::Error> {
        let mut results = Vec::new();

        let mut params: Vec<(&str, &str)> = Vec::new();

        {
            let users_cache = self.users_cache.read().unwrap();

            if let Some(logins) = logins {
                for login in logins {
                    if let Some(user) = users_cache.iter().find(|user| &user.login == *login) {
                        results.push(user.clone());
                    } else {
                        params.push(("login", login));
                    }
                }
            }
            if let Some(ids) = ids {
                for id in ids {
                    if let Some(user) = users_cache.iter().find(|user| &user.id == *id) {
                        results.push(user.clone());
                    } else {
                        params.push(("id", id));
                    }
                }
            }
        }

        let response = self
            .client
            .get("https://api.twitch.tv/helix/users")
            .headers((*self.headers).clone())
            .query(&params)
            .send()
            .await?;

        tracing::info!("GET {}: {}", response.url(), response.status());

        let api_results = response.json::<UsersResponse>().await?.data;

        if api_results.len() != 0 {
            let mut users_cache = self.users_cache.write().unwrap();

            users_cache.extend(api_results.clone());
        }

        results.extend(api_results);

        Ok(results)
    }

    pub async fn get_self_user(&self) -> Result<User, reqwest::Error> {
        Ok(self
            .get_users(None, None)
            .await?
            .into_iter()
            .next()
            .unwrap())
    }

    pub async fn run_ad(
        &self,
        channel_login: &str,
        duration: u8,
    ) -> Result<String, reqwest::Error> {
        let users = self.get_users(Some(&vec![channel_login]), None).await?;
        let channel_id = &users.first().unwrap().id;

        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("OAuth {}", self.get_oauth()).parse().unwrap(),
        );
        headers.insert("Client", self.get_client_id().to_owned().parse().unwrap());

        let mut payload = HashMap::new();
        // params.insert("channelID", channel_id);
        // params.insert("channelLogin", channel_login.to_owned());
        payload.insert("length", duration.to_string());

        let url = format!(
            "https://api.twitch.tv/v5/channels/{}/commercial",
            channel_id
        );

        Ok(self
            .client
            .post(&url)
            .headers(headers)
            .json(&payload)
            .send()
            .await?
            .text()
            .await?)
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

        tracing::debug!("{:?}", lookup);

        let mut mods = vec![channel_login.to_owned()];

        for moderator in lookup.mods {
            mods.push(moderator.login);
        }

        // Err(_) => self.get_channel_mods_from_irc(channel_login).await?,

        let mut moderators_cache = self.moderators_cache.write().unwrap();

        moderators_cache.insert(channel_login.to_string(), mods.clone());

        Ok(mods)
    }

    fn get_app_access_headers(&self) -> HeaderMap {
        let mut headers = (*self.headers).clone();

        headers.insert(
            "Authorization",
            format!(
                "Bearer {}",
                self.app_access_token
                    .as_ref()
                    .expect("App access token missing")
            )
            .parse()
            .unwrap(),
        );

        headers
    }

    pub async fn create_eventsub_subscription(
        &self,
        sub: EventsubSubscriptionType,
        secret: &str,
    ) -> Result<(), reqwest::Error> {
        let body = json!({
            "type": sub.get_name(),
            "version": sub.get_version(),
            "condition": sub.get_condition(),
            "transport": {
                "method": "webhook",
                "callback": format!("{}/webhooks/twitch", std::env::var("BASE_URL").expect("no base url")),
                "secret": secret
            }
        }).to_string();

        tracing::info!("Creating EventSub subscription {}", body);

        let pending_response = self
            .client
            .post("https://api.twitch.tv/helix/eventsub/subscriptions")
            .headers(self.get_app_access_headers())
            .body(body)
            .send()
            .await?;

        tracing::info!(
            "POST {}: {}",
            pending_response.url(),
            pending_response.status()
        );

        let text = pending_response.text().await?;

        tracing::info!("{}", text);

        Ok(())
    }

    pub async fn list_eventsub_subscriptions(
        &self,
    ) -> Result<EventsubSubscriptionList, reqwest::Error> {
        Ok(self
            .client
            .get("https://api.twitch.tv/helix/eventsub/subscriptions")
            .headers(self.get_app_access_headers())
            .send()
            .await?
            .json()
            .await?)
    }

    pub async fn delete_eventsub_subscription(&self, sub_id: &str) -> Result<(), reqwest::Error> {
        assert!(self
            .client
            .delete("https://api.twitch.tv/helix/eventsub/subscriptions")
            .query(&[("id", sub_id)])
            .headers(self.get_app_access_headers())
            .send()
            .await?
            .status()
            .is_success());

        Ok(())
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
}
