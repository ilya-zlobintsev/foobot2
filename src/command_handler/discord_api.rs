use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use twilight_http::Client;
use twilight_model::guild::Guild;
use twilight_model::id::Id;
use twilight_model::user::{CurrentUser, User};

#[derive(Clone, Debug)]
pub struct DiscordApi {
    http: Arc<Client>,
    guild_names_cache: Arc<RwLock<HashMap<u64, String>>>,
    users_cache: Arc<RwLock<HashMap<u64, User>>>,
}

impl DiscordApi {
    pub fn new(token: String) -> Self {
        let guild_names_cache = Arc::new(RwLock::new(HashMap::new()));
        let users_cache = Arc::new(RwLock::new(HashMap::new()));

        {
            let guild_names_cache = guild_names_cache.clone();
            let users_cache = users_cache.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(600)).await;

                    tracing::info!("Clearing Discord cahce");

                    let mut guild_names_cache = guild_names_cache.write().await;
                    guild_names_cache.clear();

                    let mut users_cache = users_cache.write().await;
                    users_cache.clear();
                }
            });
        }

        Self {
            http: Arc::new(Client::new(token)),
            guild_names_cache,
            users_cache,
        }
    }

    pub async fn get_self_user(&self) -> anyhow::Result<CurrentUser> {
        Ok(self.http.current_user().exec().await?.model().await?)
    }

    pub async fn get_guild(&self, guild_id: u64) -> anyhow::Result<Guild> {
        Ok(self
            .http
            .guild(Id::new(guild_id))
            .exec()
            .await?
            .model()
            .await?)
    }

    pub async fn get_guild_name(&self, guild_id: u64) -> anyhow::Result<String> {
        let guild_names_cache_guard = self.guild_names_cache.read().await;
        Ok(match guild_names_cache_guard.get(&guild_id) {
            Some(name) => name.clone(),
            None => {
                drop(guild_names_cache_guard);
                let guild = self.get_guild(guild_id).await?;
                self.guild_names_cache
                    .write()
                    .await
                    .insert(guild_id, guild.name.clone());
                guild.name
            }
        })
    }

    pub async fn get_user(&self, user_id: u64) -> anyhow::Result<User> {
        let users_cache_guard = self.users_cache.read().await;
        Ok(match users_cache_guard.get(&user_id) {
            Some(user) => user.clone(),
            None => {
                drop(users_cache_guard);
                let user = self
                    .http
                    .user(Id::new(user_id))
                    .exec()
                    .await?
                    .model()
                    .await?;
                self.users_cache.write().await.insert(user_id, user.clone());
                user
            }
        })
    }
}
