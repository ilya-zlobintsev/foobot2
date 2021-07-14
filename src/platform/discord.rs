use std::env;

use futures::StreamExt;
use twilight_gateway::{cluster::ShardScheme, Cluster, Event, Intents};
use twilight_http::Client;
use twilight_model::{gateway::payload::MessageCreate, guild::Permissions};
use twilight_util::permission_calculator::PermissionCalculator;

use crate::command_handler::CommandHandler;

use super::{ChannelIdentifier, ChatPlatform, ExecutionContext, UserIdentifier};

pub struct Discord {
    token: String,
    command_handler: CommandHandler,
    prefix: String,
}

impl Discord {
    async fn handle_msg(&self, msg: MessageCreate, http: Client) {
        tracing::debug!("{:?}", msg);
        if let Some(content) = msg.content.strip_prefix(&self.prefix) {
            let content = content.to_owned();

            let command_handler = self.command_handler.clone();
            
            tokio::spawn(async move {
                let context = DiscordExecutionContext {
                    msg: &msg,
                    http: http.clone(),
                };

                if let Some(response) = command_handler
                    .handle_command_message(&content, context)
                    .await
                {
                    http.create_message(msg.channel_id)
                        .reply(msg.id)
                        .content(response)
                        .expect("Failed to construct message")
                        .await
                        .expect("Failed to reply in Discord");
                }
            });
        }
    }
}

#[async_trait]
impl ChatPlatform for Discord {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, super::ChatPlatformError> {
        let token = env::var("DISCORD_TOKEN")?;

        Ok(Box::new(Self {
            token,
            command_handler,
            prefix: Self::get_prefix(),
        }))
    }

    async fn run(self) -> tokio::task::JoinHandle<()> {
        let scheme = ShardScheme::Auto;

        let intents = Intents::GUILD_MESSAGES | Intents::DIRECT_MESSAGES;

        let (cluster, mut events) = Cluster::builder(&self.token, intents)
            .shard_scheme(scheme)
            .build()
            .await
            .expect("Failed to connect to Discord");

        {
            let cluster = cluster.clone();

            tokio::spawn(async move {
                cluster.up().await;
            });
        }

        let http = Client::new(&self.token);

        tokio::spawn(async move {
            while let Some((_, event)) = events.next().await {
                match event {
                    Event::ShardConnected(_) => tracing::info!("Discord shard connected"),
                    Event::MessageCreate(msg) => self.handle_msg(*msg, http.clone()).await,
                    _ => (),
                }
            }
        })
    }

    fn get_prefix() -> String {
        if let Ok(prefix) = env::var("DISCORD_PREFIX") {
            prefix
        } else if let Ok(prefix) = env::var("COMMAND_PREFIX") {
            prefix
        } else {
            "!".to_string()
        }
    }
}

pub struct DiscordExecutionContext<'a> {
    msg: &'a MessageCreate,
    http: Client,
}

#[async_trait]
impl ExecutionContext for DiscordExecutionContext<'_> {
    async fn get_permissions_internal(&self) -> super::Permissions {
        match self.msg.guild_id {
            Some(guild_id) => {
                let guild_member = self
                    .http
                    .guild_member(guild_id, self.msg.author.id)
                    .await
                    .expect("Failed to get guild member")
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
                    self.msg.author.id,
                    Permissions::VIEW_CHANNEL,
                    &member_roles,
                );

                let permissions = permissions_calculator.root();

                if permissions.contains(Permissions::ADMINISTRATOR) {
                    crate::platform::Permissions::ChannelMod
                } else {
                    crate::platform::Permissions::Default
                }
            }
            None => crate::platform::Permissions::ChannelMod, // for DMs
        }
    }

    fn get_channel(&self) -> ChannelIdentifier {
        match self.msg.guild_id {
            Some(guild_id) => ChannelIdentifier::DiscordGuildID(guild_id.0.to_string()),
            None => ChannelIdentifier::Anonymous,
        }
    }

    fn get_user_identifier(&self) -> UserIdentifier {
        UserIdentifier::DiscordID(self.msg.author.id.0.to_string())
    }
}
