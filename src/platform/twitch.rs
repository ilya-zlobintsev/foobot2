use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use std::time::Instant;
use tokio::task::{self, JoinHandle};
use twitch_irc::{
    login::StaticLoginCredentials,
    message::{Badge, PrivmsgMessage, ServerMessage, TwitchUserBasics, WhisperMessage},
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

    async fn handle_message<T: 'static + TwitchMessage + Send + Sync>(&self, msg: T) {
        if let Some(message_text) = msg.get_content().strip_prefix(&self.command_prefix) {
            let message_text = message_text.to_owned();

            tracing::debug!("Recieved a command at {:?}", Instant::now());

            let client = self.client.read().unwrap().as_ref().unwrap().clone();

            let command_handler = self.command_handler.clone();

            task::spawn(async move {
                let context = TwitchExecutionContext { msg: &msg };

                let response = command_handler
                    .handle_command_message(&message_text, context)
                    .await;

                if let Some(response) = response {
                    tracing::info!("Replying with {}", response);

                    if let Some(pm) = msg.get_privmsg() {
                        client
                            .reply_to_privmsg(response, &pm)
                            .await
                            .expect("Failed to reply");
                    } else {
                        client
                            .privmsg(
                                msg.get_sender().login.clone(),
                                format!("/w {} {}", msg.get_sender().login, response),
                            )
                            .await
                            .expect("Failed to say");
                    }
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
                    ServerMessage::Privmsg(pm) => self.handle_message(pm).await,
                    ServerMessage::Whisper(whisper) => self.handle_message(whisper).await,
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

pub trait TwitchMessage {
    fn get_badges(&self) -> &Vec<Badge>;

    fn get_sender(&self) -> &TwitchUserBasics;

    fn get_channel(&self) -> Option<&str>;

    fn get_content(&self) -> &str;

    fn get_privmsg(&self) -> Option<&PrivmsgMessage>;
}

impl TwitchMessage for PrivmsgMessage {
    fn get_badges(&self) -> &Vec<Badge> {
        &self.badges
    }

    fn get_sender(&self) -> &TwitchUserBasics {
        &self.sender
    }

    fn get_channel(&self) -> Option<&str> {
        Some(&self.channel_login)
    }

    fn get_content(&self) -> &str {
        &self.message_text
    }

    fn get_privmsg(&self) -> Option<&PrivmsgMessage> {
        Some(&self)
    }
}

impl TwitchMessage for WhisperMessage {
    fn get_badges(&self) -> &Vec<Badge> {
        &self.badges
    }

    fn get_sender(&self) -> &TwitchUserBasics {
        &self.sender
    }

    fn get_channel(&self) -> Option<&str> {
        None
    }

    fn get_content(&self) -> &str {
        &self.message_text
    }

    fn get_privmsg(&self) -> Option<&PrivmsgMessage> {
        None
    }
}

pub struct TwitchExecutionContext<'a, M: TwitchMessage + std::marker::Sync> {
    msg: &'a M,
}

#[async_trait]
impl<T: TwitchMessage + std::marker::Sync> ExecutionContext for TwitchExecutionContext<'_, T> {
    async fn get_permissions_internal(&self) -> Permissions {
        if self
            .msg
            .get_badges()
            .iter()
            .any(|badge| (badge.name == "moderator") | (badge.name == "broadcaster"))
        {
            Permissions::ChannelMod
        } else {
            Permissions::Default
        }
    }

    fn get_channel(&self) -> ChannelIdentifier {
        match self.msg.get_channel() {
            Some(channel) => ChannelIdentifier::TwitchChannelName(channel.to_owned()),
            None => ChannelIdentifier::Anonymous,
        }
    }

    fn get_user_identifier(&self) -> UserIdentifier {
        UserIdentifier::TwitchID(self.msg.get_sender().id.clone())
    }
}
