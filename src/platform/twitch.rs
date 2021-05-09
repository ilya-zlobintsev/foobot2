use async_trait::async_trait;
use tokio::task;
use twitch_irc::{ClientConfig, SecureTCPTransport, TwitchIRCClient, login::StaticLoginCredentials, message::{PrivmsgMessage, ServerMessage}};

use crate::{command_handler::{CommandHandler, CommandMessage, twitch_api::TwitchApi}, platform::{ChannelIdentifier, ExecutionContext}};

use super::{ChatPlatform, UserIdentifier};

#[derive(Clone)]
pub struct Twitch {
    // client: Option<TwitchIRCClient<TCPTransport, StaticLoginCredentials>>,
    credentials: StaticLoginCredentials,
    command_handler: CommandHandler,
}

#[async_trait]
impl ChatPlatform for Twitch {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, super::ChatPlatformError> {
        let credentials = match &command_handler.twitch_api {
            Some(twitch_api) => {
                let oauth = twitch_api.get_oauth();

                let login = TwitchApi::validate_oauth(oauth).await?.login;

                tracing::info!("Logging into Twitch as {}", login);

                StaticLoginCredentials::new(login, Some(oauth.to_string()))
            }
            None => {
                tracing::info!("Twitch API not initialized! Connecting to twitch anonymously");

                StaticLoginCredentials::anonymous()
            }
        };


        Ok(Box::new(Self {
            // client: None,
            credentials,
            command_handler,
        }))
    }

    async fn run(mut self) -> () {
        let config = ClientConfig::new_simple(self.credentials.clone());

        let (mut incoming_messages, client) =
            TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(config);

        tracing::info!("Connected to Twitch");

        client.join("boring_nick".to_owned());

        async move {
            let command_prefix = Self::get_prefix();

            while let Some(message) = incoming_messages.recv().await {
                match message {
                    ServerMessage::Privmsg(mut pm) => {
                        tracing::debug!("{:?}", pm);

                        if let Some(message_text) = pm.message_text.strip_prefix(&command_prefix) {
                            pm.message_text = message_text.to_string();

                            let context = ExecutionContext {
                                channel: ChannelIdentifier::TwitchChannelID(pm.channel_id.clone()),
                            };

                            let cclient = client.clone();
                            let command_handler = self.command_handler.clone();

                            task::spawn(async move {
                                let response = command_handler.handle_command_message(&pm, context).await;

                                if let Some(response) = response {
                                    tracing::info!("Replying with {}", response);

                                    cclient
                                        .reply_to_privmsg(response, &pm)
                                        .await
                                        .expect("Failed to reply");
                                }
                            });
                        }
                    }
                    // ServerMessage::Whisper(_) => {}
                    _ => (),
                }
            }
        }.await
    }
}

impl CommandMessage for PrivmsgMessage {
    fn get_user_identifier(&self) -> UserIdentifier {
        UserIdentifier::TwitchID(self.sender.id.clone())
    }

    fn get_text(&self) -> String {
        self.message_text.clone()
    }
}
