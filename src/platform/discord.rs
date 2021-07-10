use std::env;

use futures::StreamExt;
use twilight_gateway::{cluster::ShardScheme, Cluster, Event, Intents};
use twilight_http::Client;
use twilight_model::gateway::payload::MessageCreate;

use crate::command_handler::{CommandHandler, CommandMessage};

use super::{ChannelIdentifier, ChatPlatform, ExecutionContext, UserIdentifier};

impl CommandMessage for MessageCreate {
    fn get_user_identifier(&self) -> UserIdentifier {
        UserIdentifier::DiscordID(self.author.id.0.to_string())
    }

    fn get_text(&self) -> &str {
        &self.content
    }
}

pub struct Discord {
    token: String,
    command_handler: CommandHandler,
    prefix: String,
}

impl Discord {
    async fn handle_msg(&self, mut msg: MessageCreate, http: Client) {
        if let Some(content) = msg.content.strip_prefix(&self.prefix) {
            msg.content = content.to_string();

            let context = match msg.guild_id {
                Some(guild_id) => ExecutionContext {
                    channel: ChannelIdentifier::DiscordGuildID(guild_id.0),
                    permissions: crate::platform::Permissions::Default, // TODO
                },
                None => ExecutionContext {
                    channel: ChannelIdentifier::DiscordChannelID(msg.channel_id.0),
                    permissions: crate::platform::Permissions::ChannelMod,
                },
            };

            if let Some(response) = self
                .command_handler
                .handle_command_message(&msg, context)
                .await
            {
                http.create_message(msg.channel_id)
                    .content(response)
                    .expect("Failed to construct message")
                    .await
                    .expect("Failed to reply in Discord");
            }
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

        let (cluster, mut events) = Cluster::builder(&self.token, Intents::GUILD_MESSAGES)
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
