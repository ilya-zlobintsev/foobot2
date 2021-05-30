pub mod model;

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use reqwest::{header::HeaderMap, Client};
use tokio::task;
use twitch_irc::{
    login::StaticLoginCredentials, message::ServerMessage, ClientConfig, SecureTCPTransport,
    TwitchIRCClient,
};

use model::*;

#[derive(Clone)]
pub struct TwitchApi {
    headers: HeaderMap,
    client: Client,
    moderators_cache: Arc<RwLock<HashMap<String, Vec<String>>>>,
    users_cache: Arc<RwLock<Vec<User>>>,
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

        let moderators_cache = Arc::new(RwLock::new(HashMap::new()));

        let users_cache = Arc::new(RwLock::new(Vec::new()));

        let twitch_api = TwitchApi {
            headers,
            client: Client::new(),
            moderators_cache,
            users_cache,
        };

        twitch_api.start_cron().await;

        Ok(twitch_api)
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
        tracing::info!("Validating twitch API token: {}", response.status());
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

        let api_results = self
            .client
            .get("https://api.twitch.tv/helix/users")
            .headers(self.headers.clone())
            .query(&params)
            .send()
            .await?
            .json::<UsersResponse>()
            .await?
            .data;

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

        let mods = match self
            .client
            .get(format!(
                "https://api.ivr.fi/twitch/modsvips/{}",
                channel_login
            ))
            .send()
            .await
        {
            Ok(response) => {
                tracing::info!("GET {}: {}", response.url(), response.status());

                let lookup = response.json::<IvrModInfo>().await?;
                
                tracing::debug!("{:?}", lookup);

                let mut mods = vec![channel_login.to_owned()];

                for moderator in lookup.mods {
                    mods.push(moderator.login);
                }
                
                mods
            }
            Err(_) => self.get_channel_mods_from_irc(channel_login).await?,
        };

        let mut moderators_cache = self.moderators_cache.write().unwrap();

        moderators_cache.insert(channel_login.to_string(), mods.clone());

        Ok(mods)
    }

    // This terrible abomination has to exist because twitch doesn't provide an endpoint for this that doesn't require channel auth
    /// Returns the list of logins of channel moderators. Don't expect this to be efficient
    async fn get_channel_mods_from_irc(
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
    }
}
