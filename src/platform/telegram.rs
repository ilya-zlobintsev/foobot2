use super::{
    ChannelIdentifier, ChatPlatform, ChatPlatformError, ExecutionContext, Permissions,
    UserIdentifier,
};
use crate::command_handler::CommandHandler;
use frankenstein::{
    AsyncApi, AsyncTelegramApi, GetUpdatesParamsBuilder, Message, SendMessageParamsBuilder,
};
use std::env;
use std::sync::Arc;

pub struct Telegram {
    api: AsyncApi,
    prefix: Arc<String>,
    command_handler: CommandHandler,
}

#[async_trait]
impl ChatPlatform for Telegram {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, ChatPlatformError> {
        let token = env::var("TELEGRAM_TOKEN")
            .map_err(|_| ChatPlatformError::MissingEnv(String::from("TELEGRAM_TOKEN")))?;

        let api = AsyncApi::new(&token);
        let prefix = Arc::new(Self::get_prefix());

        Ok(Box::new(Self {
            api,
            prefix,
            command_handler,
        }))
    }

    async fn run(self) {
        let mut update_params_builder = GetUpdatesParamsBuilder::default();
        update_params_builder.allowed_updates(vec!["message".to_string(), "channel_post".to_string()]);

        let mut update_params = update_params_builder.build().unwrap();

        tokio::spawn(async move {
            loop {
                let result = self.api.get_updates(&update_params).await;
                match result {
                    Ok(response) => {
                        for update in response.result {
                            tracing::trace!("Update: {:?}", update);
                            if let Some(message) = update.message {
                                if let Some(message_text) = message.text.clone() {
                                    let prefix = self.prefix.clone();

                                    let mut display_name = String::new();
                                    {
                                        let from = message.from.as_ref().unwrap();

                                        display_name.push_str(&from.first_name);
                                        if let Some(last_name) = &from.last_name {
                                            display_name.push_str(" ");
                                            display_name.push_str(&last_name);
                                        }
                                    }

                                    let command_handler = self.command_handler.clone();
                                    let api = self.api.clone();

                                    tokio::spawn(async move {
                                        let context = TelegramExectuionContext {
                                            msg: &message,
                                            display_name,
                                            prefix,
                                        };

                                        if let Some(response) = command_handler
                                            .handle_message(&message_text, context)
                                            .await
                                        {
                                            let send_message_params =
                                                SendMessageParamsBuilder::default()
                                                    .chat_id(message.chat.id)
                                                    .text(&response)
                                                    .reply_to_message_id(message.message_id)
                                                    .build()
                                                    .unwrap();

                                            if let Err(e) =
                                                api.send_message(&send_message_params).await
                                            {
                                                tracing::error!(
                                                    "Error responding on telegram: {:?}",
                                                    e
                                                );
                                            }
                                        }
                                    });
                                }
                                update_params = update_params_builder
                                    .offset(update.update_id + 1)
                                    .build()
                                    .unwrap();
                            }
                        }
                    }
                    Err(e) => tracing::warn!("Failed to get Telegram updates: {:?}", e),
                }
            }
        });
    }
}

pub struct TelegramExectuionContext<'a> {
    msg: &'a Message,
    display_name: String,
    prefix: Arc<String>,
}

#[async_trait]
impl ExecutionContext for TelegramExectuionContext<'_> {
    async fn get_permissions_internal(&self) -> Permissions {
        match &self.msg.chat.permissions {
            Some(permissions) => {
                if permissions.can_change_info == Some(true)
                    && permissions.can_pin_messages == Some(true)
                {
                    Permissions::ChannelMod
                } else {
                    Permissions::Default
                }
            }
            None => Permissions::Default,
        }
    }

    fn get_channel(&self) -> ChannelIdentifier {
        ChannelIdentifier::TelegramChatId(self.msg.chat.id.to_string())
    }

    fn get_user_identifier(&self) -> UserIdentifier {
        UserIdentifier::TelegramId(self.msg.from.as_ref().expect("No message sender?").id)
    }

    fn get_display_name(&self) -> &str {
        &self.display_name
    }

    fn get_prefixes(&self) -> Vec<&str> {
        vec![&self.prefix]
    }
}
