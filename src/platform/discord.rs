use std::env;

use super::{ChannelIdentifier, ChatPlatform, ExecutionContext, UserIdentifier};

use crate::command_handler::{CommandHandler, CommandMessage};

use async_trait::async_trait;
use serenity::{
    client::{validate_token, Context, EventHandler},
    model::{channel::Message, prelude::Ready},
    prelude::TypeMapKey,
    Client,
};

struct Handler {
    prefix: String,
}

#[async_trait]
impl EventHandler for Handler {
    // Event handlers are dispatched through a threadpool, and so multiple
    // events can be dispatched simultaneously.
    async fn message(&self, ctx: Context, mut message: Message) {
        if message.content.starts_with(&self.prefix) {
            message.content = message.content.strip_prefix(&self.prefix).unwrap().to_string();

            let data = ctx.data.read().await;

            let command_handler = data
                .get::<CommandHandler>()
                .expect("CommandHandler not found in client");

            let command_context = ExecutionContext {
                channel: ChannelIdentifier::DiscordGuildID(message.guild_id.unwrap().to_string()), // TODO
            };

            if let Some(response) = command_handler
                .handle_command_message(&message, command_context)
                .await
            {
                tracing::info!("Replying with {}", response);

                message
                    .channel_id
                    .say(&ctx.http, response)
                    .await
                    .expect("Failed to send message");
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        tracing::info!("Connected to discord as {}", ready.user.name);
    }
}

pub struct Discord {
    client: Client,
}

#[async_trait]
impl ChatPlatform for Discord {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, super::ChatPlatformError> {
        let token = env::var("DISCORD_TOKEN")?;

        validate_token(&token)?;

        let client = Client::builder(token)
            .event_handler(Handler {
                prefix: Self::get_prefix(),
            })
            .await?;

        {
            let mut data = client.data.write().await;

            data.insert::<CommandHandler>(command_handler);
        }

        Ok(Box::new(Self { client }))
    }

    async fn run(mut self) -> () {
        self.client.start().await.expect("discord error")
    }
}

impl TypeMapKey for CommandHandler {
    type Value = CommandHandler;
}

impl CommandMessage for Message {
    fn get_user_identifier(&self) -> super::UserIdentifier {
        UserIdentifier::DiscordID(self.author.id.as_u64().to_string())
    }

    fn get_text(&self) -> String {
        self.content.clone()
    }
}
