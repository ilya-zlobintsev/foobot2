use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use twilight_http::Client;
use twilight_model::guild::Permissions;
use twilight_model::id::{GuildId, UserId};
use twilight_model::user::CurrentUser;
use twilight_util::permission_calculator::PermissionCalculator;

#[derive(Clone)]
pub struct DiscordApi {
    http: Client,
    permissions_cache: Arc<RwLock<HashMap<(u64, u64), Permissions>>>, // (guild_id, user_id)
}

impl DiscordApi {
    pub fn new(token: &str) -> Self {
        let permissions_cache = Arc::new(RwLock::new(HashMap::new()));

        {
            let permissions_cache = permissions_cache.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(600)).await;

                    tracing::info!("Clearing Discord cahce");

                    let mut permissions_cache = permissions_cache.write().await;

                    permissions_cache.clear();
                }
            });
        }

        Self {
            http: Client::new(token),
            permissions_cache,
        }
    }

    pub async fn get_self_user(&self) -> Result<CurrentUser, twilight_http::Error> {
        self.http.current_user().await
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

                Ok(permissions.clone())
            },
            None => {
                drop(permissions_cache);
                
                tracing::debug!("Querying user permissions");

                let user_id = UserId(user_id);
                let guild_id = GuildId(guild_id);

                let guild_member = self
                    .http
                    .guild_member(guild_id, user_id)
                    .await?
                    .expect("Not a guild member");

                let guild_roles = self
                    .http
                    .roles(guild_id)
                    .await
                    .expect("Failed to get guild roles");

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

                permissions_cache.insert((guild_id.0, user_id.0), permissions);

                Ok(permissions)
            }
        }
    }
}

enum CacheKind {
    GuildPermissions { guild_id: u64, user_id: u64 },
}
