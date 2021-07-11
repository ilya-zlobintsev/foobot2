use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use std::time::Instant;
use tokio::task::{self, JoinHandle};
use twitch_irc::{
    login::StaticLoginCredentials,
    message::{PrivmsgMessage, ServerMessage},
    ClientConfig, SecureTCPTransport, TwitchIRCClient,
};

use crate::{
    command_handler::{twitch_api::TwitchApi, CommandHandler},
    platform::{ChannelIdentifier, ExecutionContext, Permissions},
};

use super::{ChatPlatform, UserIdentifier};

#[derive(Clone)]
pub struct Twitch {
    client: Arc<RwLock<Option<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>>>,
    command_handler: CommandHandler,
    command_prefix: String,
    credentials: StaticLoginCredentials,
}

impl Twitch {
    pub fn join_channel(&self, channel: String) {
        let client = self.client.read().unwrap();
        let client = client.as_ref().unwrap();

        client.join(channel);
    }

    async fn handle_privmsg(&self, pm: PrivmsgMessage) {
        let command_prefix = match std::env::var(format!("PREFIX_TWITCH_{}", pm.channel_login)) {
            Ok(prefix) => prefix,
            Err(_) => self.command_prefix.clone(), // TODO
        };

        if let Some(message_text) = pm.message_text.strip_prefix(&command_prefix) {
            let message_text = message_text.to_owned();

            tracing::debug!("Recieved a command at {:?}", Instant::now());

            let client = self.client.read().unwrap().as_ref().unwrap().clone();

            let command_handler = self.command_handler.clone();

            task::spawn(async move {
                let context = TwitchExecutionContext { pm: &pm };

                let response = command_handler
                    .handle_command_message(&message_text, context)
                    .await;

                if let Some(response) = response {
                    tracing::info!("Replying with {}", response);

                    client
                        .reply_to_privmsg(response, &pm)
                        .await
                        .expect("Failed to reply");
                }
            });
        }
    }
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

        let command_prefix = Self::get_prefix();

        Ok(Box::new(Self {
            client: Arc::new(RwLock::new(None)),
            command_handler,
            command_prefix,
            credentials,
        }))
    }

    async fn run(self) -> JoinHandle<()> {
        tracing::info!("Connected to Twitch");

        let config = ClientConfig::new_simple(self.credentials.clone());

        let (mut incoming_messages, client) =
            TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(config);

        *self.client.write().unwrap() = Some(client.clone());

        tokio::spawn(async move {
            while let Some(message) = incoming_messages.recv().await {
                match message {
                    ServerMessage::Privmsg(pm) => self.handle_privmsg(pm).await,
                    // ServerMessage::Whisper(_) => {}
                    _ => (),
                }
            }
        })
    }

    /*async fn create_listener(&self) -> std::sync::mpsc::Sender<super::PlatformMessage> {
        let (sender, receiver): (Sender<PlatformMessage>, Receiver<PlatformMessage>) = channel();

        let client = self.client.read().unwrap().as_ref().unwrap().clone();

        task::spawn(async move {
            loop {
                let msg = receiver.recv().unwrap();

                client
                    .privmsg(msg.channel_id, msg.message)
                    .await
                    .expect("Twitch error");
            }
        });

        sender
    }*/
}

pub struct TwitchExecutionContext<'a> {
    pm: &'a PrivmsgMessage,
}

#[async_trait]
impl ExecutionContext for TwitchExecutionContext<'_> {
    async fn get_permissions_internal(&self) -> Permissions {
        if self.pm.badges.iter().any(|badge| badge.name == "moderator")
            | self
                .pm
                .badges
                .iter()
                .any(|badge| badge.name == "broadcaster")
        {
            Permissions::ChannelMod
        } else {
            Permissions::Default
        }
    }

    fn get_channel(&self) -> ChannelIdentifier {
        ChannelIdentifier::TwitchChannelName(self.pm.channel_login.clone())
    }

    fn get_user_identifier(&self) -> UserIdentifier {
        UserIdentifier::TwitchID(self.pm.sender.id.clone())
    }
}
