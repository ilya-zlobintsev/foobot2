use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::{anyhow, Context};
use http::{HeaderMap, Method};
use reqwest::{Client, RequestBuilder, Response};
use tokio::task;
use twitch_irc::login::{LoginCredentials, StaticLoginCredentials};

use crate::{command_handler::twitch_api::model::UsersResponse, web::response_ok};

use super::{get_client_id, model::*};

pub const HELIX_URL: &str = "https://api.twitch.tv/helix";

#[derive(Clone, Debug)]
pub struct HelixApi<C: LoginCredentials> {
    client: Client,
    pub credentials: C,
    users_cache: Arc<RwLock<Vec<User>>>,
    headers: HeaderMap,
}

impl<C: LoginCredentials> HelixApi<C> {
    pub async fn with_credentials(credentials: C) -> Self {
        let mut headers = HeaderMap::new();

        headers.insert(
            "Client-Id",
            get_client_id().expect("Client ID missing").parse().unwrap(),
        );

        let helix = Self {
            client: Client::new(),
            credentials,
            users_cache: Arc::new(RwLock::new(Vec::new())),
            headers,
        };

        helix.start_cron().await;

        helix
    }

    pub async fn start_cron(&self) {
        let users_cache = self.users_cache.clone();

        task::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(3600)).await;

                tracing::info!("Clearing users cache");

                let mut users_cache = users_cache.write().expect("Failed to lock");

                users_cache.clear();
            }
        });
    }

    async fn request(&self, method: Method, path: &str) -> anyhow::Result<RequestBuilder> {
        let credentials = self
            .credentials
            .get_credentials()
            .await
            .map_err(|_| anyhow!("Failed to get credentials"))?;

        Ok(self
            .client
            .request(method, format!("{}{}", HELIX_URL, path))
            .headers(self.headers.clone())
            .bearer_auth(&credentials.token.context("Token missing")?))
    }

    async fn get(&self, path: &str) -> anyhow::Result<RequestBuilder> {
        self.request(Method::GET, path).await
    }

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
                    if let Some(user) = users_cache.iter().find(|user| user.login == *login) {
                        tracing::info!("Using cache for user {}", user.login);
                        results.push(user.clone());
                    } else {
                        params.push(("login", login));
                    }
                }
            }
            if let Some(ids) = ids {
                for id in ids {
                    if let Some(user) = users_cache.iter().find(|user| user.id == *id) {
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

                    if !api_results.is_empty() {
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

    pub async fn get_user_by_id(&self, id: &str) -> anyhow::Result<User> {
        let users = self.get_users(None, Some(&vec![id])).await?;

        users
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("User not found"))
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

    /// Returns a list of Custom Reward objects for the Custom Rewards on the authenticated user's channel.
    pub async fn get_custom_rewards(&self) -> anyhow::Result<Vec<CustomReward>> {
        let broadcaster_id = self.get_self_user().await?.id;

        let response = self
            .get("/channel_points/custom_rewards")
            .await?
            .query(&[("broadcaster_id", broadcaster_id)])
            .send()
            .await?;

        response_ok(&response)?;
        
        Ok(response.json().await?)
    }
}

impl HelixApi<StaticLoginCredentials> {
    pub async fn with_token(access_token: &str) -> anyhow::Result<Self> {
        Ok(Self::with_credentials(StaticLoginCredentials::new(
            String::new(),
            Some(access_token.to_owned()),
        ))
        .await)
    }
}