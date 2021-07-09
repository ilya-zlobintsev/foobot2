use std::env;

use crate::command_handler::{CommandHandler, CommandMessage};

use super::{ChannelIdentifier, ChatPlatform, ChatPlatformError, ExecutionContext, UserIdentifier};
use discord::model::{Event, Message};

impl CommandMessage for Message {
    fn get_user_identifier(&self) -> UserIdentifier {
        UserIdentifier::DiscordID(self.author.id.0.to_string())
    }

    fn get_text(&self) -> &str {
        &self.content
    }
}

pub struct Discord {
    discord: discord::Discord,
    command_handler: CommandHandler,
    prefix: String,
}

impl Discord {
    async fn handle_message(&self, mut message: Message) {
        if let Some(command) = message.content.strip_prefix(&self.prefix) {
            message.content = command.to_string();

            let context = ExecutionContext {
                channel: ChannelIdentifier::DiscordChannelID(message.channel_id.0),
                permissions: super::Permissions::Default, // TODO
            };

            if let Some(response) = self
                .command_handler
                .handle_command_message(&message, context)
                .await
            {
                self.discord
                    .send_message(message.channel_id, &response, "", false)
                    .expect("Failed to reply on Discord");
            }
        }
    }
}

#[async_trait]
impl ChatPlatform for Discord {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, super::ChatPlatformError> {
        let token = env::var("DISCORD_TOKEN")?;

        let discord = discord::Discord::from_bot_token(&token)
            .map_err(|_| ChatPlatformError::DiscordError)?;

        Ok(Box::new(Self {
            discord,
            command_handler,
            prefix: Self::get_prefix(),
        }))
    }

    async fn run(self) -> tokio::task::JoinHandle<()> {
        let (mut connection, _) = self
            .discord
            .connect()
            .expect("Failed to connect to Discord");

        tokio::spawn(async move {
            loop {
                match connection.recv_event() {
                    Ok(Event::MessageCreate(message)) => self.handle_message(message).await,
                    Ok(_) => {}
                    Err(err) => tracing::info!("Discord connection closed with {}", err),
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
