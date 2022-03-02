use std::{env, sync::Arc};

use futures::StreamExt;
use twilight_gateway::{cluster::ShardScheme, Cluster, Event, Intents};
use twilight_http::Client;
use twilight_model::{gateway::payload::incoming::MessageCreate, guild::Permissions};

use crate::command_handler::CommandHandler;

use super::{ChannelIdentifier, ChatPlatform, ExecutionContext, UserIdentifier};

#[derive(Clone)]
pub struct Discord {
    token: String,
    command_handler: CommandHandler,
    prefix: Arc<String>,
    self_mention: Arc<String>,
}

impl Discord {
    async fn handle_msg(&self, msg: MessageCreate, http: Arc<Client>) {
        tracing::debug!("{:?}", msg);

        let Self {
            command_handler,
            prefix,
            self_mention,
            ..
        } = self.clone();

        tokio::spawn(async move {
            let context = DiscordExecutionContext {
                msg: &msg,
                cmd: &command_handler,
                prefix,
                self_mention,
            };

            if let Some(response) = command_handler.handle_message(&msg.content, context).await {
                http.create_message(msg.channel_id)
                    .reply(msg.id)
                    .content(&response)
                    .expect("Failed to construct message")
                    .exec()
                    .await
                    .expect("Failed to reply in Discord");
            }
        });
    }
}

#[async_trait]
impl ChatPlatform for Discord {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, super::ChatPlatformError> {
        let token = env::var("DISCORD_TOKEN")?;

        Ok(Box::new(Self {
            token,
            command_handler,
            prefix: Arc::new(Self::get_prefix()),
            self_mention: Arc::new(format!(
                "<@!{}>",
                env::var("DISCORD_CLIENT_ID").expect("DISCORD_CLIENT_ID not specified")
            )),
        }))
    }

    async fn run(self) {
        let scheme = ShardScheme::Auto;

        let intents = Intents::GUILD_MESSAGES | Intents::DIRECT_MESSAGES;

        let (cluster, mut events) = Cluster::builder(self.token.clone(), intents)
            .shard_scheme(scheme)
            .build()
            .await
            .expect("Failed to connect to Discord");

        {
            tokio::spawn(async move {
                cluster.up().await;
            });
        }

        let http = Arc::new(Client::new(self.token.clone()));

        tokio::spawn(async move {
            while let Some((_, event)) = events.next().await {
                match event {
                    Event::ShardConnected(_) => tracing::info!("Discord shard connected"),
                    Event::MessageCreate(msg) => self.handle_msg(*msg, http.clone()).await,
                    _ => (),
                }
            }
        });
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

#[derive(Clone)]
pub struct DiscordExecutionContext<'a> {
    msg: &'a MessageCreate,
    cmd: &'a CommandHandler,
    prefix: Arc<String>,
    self_mention: Arc<String>,
}

#[async_trait]
impl ExecutionContext for DiscordExecutionContext<'_> {
    async fn get_permissions_internal(&self) -> super::Permissions {
        tracing::info!(
            "Querying permissions for Discord user {}",
            self.msg.author.id
        );

        match self.msg.guild_id {
            Some(guild_id) => {
                let platform_handler = self.cmd.platform_handler.read().await;

                let permissions = platform_handler
                    .discord_api
                    .as_ref()
                    .unwrap()
                    .get_permissions_in_guild(self.msg.author.id.get(), guild_id.get())
                    .await
                    .expect("Failed to get permissions");

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
            Some(guild_id) => ChannelIdentifier::DiscordChannel(guild_id.to_string()),
            None => ChannelIdentifier::Anonymous,
        }
    }

    fn get_user_identifier(&self) -> UserIdentifier {
        UserIdentifier::DiscordID(self.msg.author.id.to_string())
    }

    fn get_display_name(&self) -> &str {
        &self.msg.author.name
    }

    fn get_prefixes(&self) -> Vec<&str> {
        vec![&self.prefix, &self.self_mention]
    }
}
