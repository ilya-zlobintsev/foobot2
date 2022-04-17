use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use twilight_http::Client;
use twilight_model::guild::{Guild, Permissions};
use twilight_model::id::Id;
use twilight_model::user::{CurrentUser, User};
use twilight_util::permission_calculator::PermissionCalculator;

#[derive(Clone, Debug)]
pub struct DiscordApi {
    http: Arc<Client>,
    permissions_cache: Arc<RwLock<HashMap<(u64, u64), Permissions>>>, // (guild_id, user_id)
    guild_names_cache: Arc<RwLock<HashMap<u64, String>>>,
    users_cache: Arc<RwLock<HashMap<u64, User>>>,
}

impl DiscordApi {
    pub fn new(token: String) -> Self {
        let permissions_cache = Arc::new(RwLock::new(HashMap::new()));
        let guild_names_cache = Arc::new(RwLock::new(HashMap::new()));
        let users_cache = Arc::new(RwLock::new(HashMap::new()));

        {
            let permissions_cache = permissions_cache.clone();
            let guild_names_cache = guild_names_cache.clone();
            let users_cache = users_cache.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(600)).await;

                    tracing::info!("Clearing Discord cahce");

                    let mut permissions_cache = permissions_cache.write().await;
                    permissions_cache.clear();

                    let mut guild_names_cache = guild_names_cache.write().await;
                    guild_names_cache.clear();

                    let mut users_cache = users_cache.write().await;
                    users_cache.clear();
                }
            });
        }

        Self {
            http: Arc::new(Client::new(token)),
            permissions_cache,
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

    pub async fn get_permissions_in_guild(
        &self,
        user_id: u64,
        guild_id: u64,
    ) -> Result<Permissions, twilight_http::Error> {
        let permissions_cache = self.permissions_cache.read().await;

        match permissions_cache.get(&(guild_id, user_id)) {
            Some(permissions) => {
                tracing::debug!("Using cached permissions");

                Ok(*permissions)
            }
            None => {
                drop(permissions_cache);

                tracing::debug!("Querying user permissions");

                let user_id = Id::new(user_id);
                let guild_id = Id::new(guild_id);

                let guild_member = self
                    .http
                    .guild_member(guild_id, user_id)
                    .exec()
                    .await?
                    .model()
                    .await
                    .unwrap();

                let guild_roles = self
                    .http
                    .roles(guild_id)
                    .exec()
                    .await?
                    .model()
                    .await
                    .unwrap();

                let mut member_roles = Vec::new();

                for role in guild_member.roles {
                    let role = guild_roles
                        .iter()
                        .find(|guild_role| guild_role.id == role)
                        .expect("Failed to get role");

                    member_roles.push((role.id, role.permissions));
                }

                let permissions_calculator = PermissionCalculator::new(
                    guild_id,
                    user_id,
                    Permissions::VIEW_CHANNEL,
                    &member_roles,
                );

                let permissions = permissions_calculator.root();

                let mut permissions_cache = self.permissions_cache.write().await;

                permissions_cache.insert((guild_id.get(), user_id.get()), permissions);

                Ok(permissions)
            }
        }
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
