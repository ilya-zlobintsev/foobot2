use std::env;

use super::{ChannelIdentifier, ChatPlatform, ExecutionContext, Permissions, UserIdentifier};

use crate::command_handler::{CommandHandler, CommandMessage};

use async_trait::async_trait;
use serenity::{
    client::{validate_token, Context, EventHandler},
    model::{channel::Message, prelude::Ready},
    prelude::TypeMapKey,
    Client,
};
use tokio::task::JoinHandle;

struct Handler {
    prefix: String,
}

#[async_trait]
impl EventHandler for Handler {
    // Event handlers are dispatched through a threadpool, and so multiple
    // events can be dispatched simultaneously.
    async fn message(&self, ctx: Context, mut message: Message) {
        tracing::debug!("{:?}", message);

        if message.content.starts_with(&self.prefix) {
            message.content = message
                .content
                .strip_prefix(&self.prefix)
                .unwrap()
                .to_string();

            let data = ctx.data.read().await;

            let command_handler = data
                .get::<CommandHandler>()
                .expect("CommandHandler not found in client");

            let command_context = ExecutionContext {
                channel: match message.guild_id {
                    Some(guild_id) => {
                        ChannelIdentifier::DiscordGuildID(guild_id.as_u64().to_string())
                    }
                    None => {
                        ChannelIdentifier::DiscordChannelID(message.channel_id.as_u64().to_string())
                    }
                },
                permissions: {
                    match message.guild_id {
                        Some(_guild_id) => {
                            // let guild = ctx.http.get_guild(guild_id.0).await.expect("Failed to get guild ID");

                            // let channel = guild.channels(ctx.http).await.unwrap().get(&message.channel_id).unwrap();
                            // let member = guild.member(ctx.http, message.author.id).await.unwrap();

                            // let permissions = guild.user_permissions_in(channel, &member).unwrap();

                            // TODO

                            Permissions::Default
                        }
                        None => Permissions::ChannelMod, // in direct messages
                    }
                },
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
    token: String,
    command_handler: CommandHandler,
}

#[async_trait]
impl ChatPlatform for Discord {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, super::ChatPlatformError> {
        let token = env::var("DISCORD_TOKEN")?;

        validate_token(&token)?;

        Ok(Box::new(Self { token, command_handler }))
    }

    async fn run(self) -> JoinHandle<()> {

        let mut client = Client::builder(self.token.clone())
            .event_handler(Handler {
                prefix: Self::get_prefix(),
            })
            .await.expect("Failed to start Discord");

        {
            let mut data = client.data.write().await;

            data.insert::<CommandHandler>(self.command_handler.clone());
        }

        tokio::spawn(async move {client.start().await.expect("Discord error") })
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
