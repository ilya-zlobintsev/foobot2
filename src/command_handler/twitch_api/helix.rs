use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::{anyhow, Context};
use http::{HeaderMap, Method};
use reqwest::{Client, RequestBuilder};
use serde_json::{json, Value};
use tokio::task;
use twitch_irc::login::{LoginCredentials, StaticLoginCredentials};

use crate::{api::response_ok, command_handler::twitch_api::model::UsersResponse};

use super::{
    eventsub::{EventSubSubscription, EventSubSubscriptionResponse, EventSubSubscriptionType},
    get_client_id,
    model::*,
};

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
                tokio::time::sleep(Duration::from_secs(36000)).await;

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
            .bearer_auth(credentials.token.context("Token missing")?))
    }

    async fn get(&self, path: &str) -> anyhow::Result<RequestBuilder> {
        self.request(Method::GET, path).await
    }

    async fn post(&self, path: &str) -> anyhow::Result<RequestBuilder> {
        self.request(Method::POST, path).await
    }

    async fn delete(&self, path: &str) -> anyhow::Result<RequestBuilder> {
        self.request(Method::DELETE, path).await
    }

    pub async fn get_users(
        &self,
        logins: Option<&[&str]>,
        ids: Option<&[&str]>,
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
            let response = self.get("/users").await?.query(&params).send().await?;

            tracing::info!("GET {}: {}", response.url(), response.status());

            response_ok(&response)?;

            let api_results = response.json::<UsersResponse>().await?.data;

            if !api_results.is_empty() {
                let mut users_cache = self.users_cache.write().unwrap();

                users_cache.extend(api_results.clone());
            }

            results.extend(api_results);
        }

        Ok(results)
    }

    pub async fn get_user_by_id(&self, id: &str) -> anyhow::Result<User> {
        let users = self.get_users(None, Some(&vec![id])).await?;

        users
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("User not found"))
    }

    /*async fn get_token(&self) -> anyhow::Result<String> {
        Ok(self
            .credentials
            .get_credentials()
            .await
            .map_err(|e| anyhow!("Unable to get credentials: {:?}", e))?
            .token
            .ok_or_else(|| anyhow!("Token missing"))?)
    }*/

    pub async fn get_self_user(&self) -> anyhow::Result<User> {
        Ok(self
            .get_users(None, None)
            .await?
            .into_iter()
            .next()
            .unwrap())
    }

    /// Returns a list of Custom Reward objects for the Custom Rewards on the authenticated user's channel.
    pub async fn get_custom_rewards(&self) -> anyhow::Result<CustomRewardsResponse> {
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

    pub async fn add_eventsub_subscription(
        &self,
        subscription: EventSubSubscriptionType,
    ) -> anyhow::Result<EventSubSubscriptionResponse> {
        let response = self
            .post("/eventsub/subscriptions")
            .await?
            .json(&subscription.build_body())
            .send()
            .await?;

        if let Err(e) = response_ok(&response) {
            let text = response.text().await?;
            tracing::info!("{}", text);

            return Err(e);
        }

        tracing::info!("Succesfully added EventSub subscription {:?}", subscription);

        Ok(response.json().await?)
    }

    pub async fn get_eventsub_subscriptions(
        &self,
        sub_type: Option<&str>,
    ) -> anyhow::Result<Vec<EventSubSubscription>> {
        let mut request = self.get("/eventsub/subscriptions").await?;

        if let Some(sub_type) = sub_type {
            request = request.query(&[("type", sub_type)]);
        }

        let response = request.send().await?;

        let mut v: Value = response.json().await?;

        let data = v["data"].take();

        Ok(serde_json::from_value(data)?)
    }

    pub async fn delete_eventsub_subscription(&self, id: &str) -> anyhow::Result<()> {
        response_ok(
            &self
                .delete("/eventsub/subscriptions")
                .await?
                .query(&[("id", id)])
                .send()
                .await?,
        )
    }

    pub async fn start_commercial(&self, length: i32) -> anyhow::Result<StartCommercialInfo> {
        if !(30..=180).contains(&length) || (length % 30 != 0) {
            return Err(anyhow!(
                "invalid commercial length! Valid options are 30, 60, 90, 120, 150, 180."
            ));
        }

        let broadcaster_id = self.get_self_user().await?.id;

        let payload = json!({
            "broadcaster_id": broadcaster_id,
            "length": length
        });

        let response = self
            .post("/channels/commercial")
            .await?
            .json(&payload)
            .send()
            .await?;

        response_ok(&response)?;

        let mut data = response
            .json::<GenericHelixResponse<StartCommercialInfo>>()
            .await?;

        let info = data.data.remove(0);

        assert_eq!(info.length, length);

        Ok(info)
    }

    pub async fn ban_user(
        &self,
        broadcaster_id: &str,
        user_id: &str,
        duration: Option<i32>,
    ) -> anyhow::Result<()> {
        let self_id = self.get_self_user().await?.id;
        debug!("Self id: {self_id}");

        let payload = json!({
            "data": {
                "user_id": user_id,
                "duration": duration,
            }
        });
        debug!("Timeout payload: {payload}");

        let response = self
            .post("/moderation/bans")
            .await?
            .query(&[
                ("broadcaster_id", broadcaster_id),
                ("moderator_id", &self_id),
            ])
            .json(&payload)
            .send()
            .await?;

        response_ok(&response)?;

        Ok(())
    }

    pub async fn ban_user_by_name(
        &self,
        broadcaster_id: &str,
        user_name: &str,
        duration: Option<i32>,
    ) -> anyhow::Result<()> {
        let users = self.get_users(Some(&[user_name]), None).await?;
        debug!("Fetched user info");
        let user = users.first().context("Empty users response")?;
        self.ban_user(broadcaster_id, &user.id, duration).await
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
