use std::env;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::time::Instant;
use tokio::task::{self, JoinHandle};
use twitch_irc::login::{RefreshingLoginCredentials, TokenStorage, UserAccessToken};
use twitch_irc::message::{Badge, PrivmsgMessage, ServerMessage, TwitchUserBasics, WhisperMessage};
use twitch_irc::{ClientConfig, SecureTCPTransport, TwitchIRCClient};

use crate::command_handler::{twitch_api::TwitchApi, CommandHandler};
use crate::database::DatabaseError;
use crate::platform::{ChannelIdentifier, ExecutionContext};

use super::{ChatPlatform, Permissions, UserIdentifier};

#[derive(Clone)]
pub struct Twitch {
    client: Arc<
        RwLock<
            Option<TwitchIRCClient<SecureTCPTransport, RefreshingLoginCredentials<CommandHandler>>>,
        >,
    >,
    command_handler: CommandHandler,
    command_prefix: String,
}

impl Twitch {
    pub fn join_channel(&self, channel: String) {
        tracing::info!("Joining channel {}", channel);

        let client = self.client.read().unwrap();
        let client = client.as_ref().unwrap();

        client.join(channel);
    }

    async fn handle_message<T: 'static + TwitchMessage + Send + Sync>(&self, msg: T) {
        if let Some(message_text) = msg.get_content().strip_prefix(&self.command_prefix) {
            let message_text = message_text.to_owned();

            let recieved_instant = Instant::now();

            tracing::debug!("Recieved a command at {:?}", recieved_instant);

            let client = self.client.read().unwrap().as_ref().unwrap().clone();

            let command_handler = self.command_handler.clone();

            task::spawn(async move {
                let context = TwitchExecutionContext { msg: &msg };

                let response = command_handler
                    .handle_command_message(&message_text, context)
                    .await;

                tracing::debug!(
                    "Command took {}ms to process",
                    recieved_instant.elapsed().as_millis()
                );

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
        let command_prefix = Self::get_prefix();

        Ok(Box::new(Self {
            client: Arc::new(RwLock::new(None)),
            command_handler,
            command_prefix,
        }))
    }

    async fn run(self) -> JoinHandle<()> {
        let login = env::var("TWITCH_LOGIN_NAME").expect("TWITCH_LOGIN_NAME missing");
        let client_id = TwitchApi::get_client_id().expect("Twitch client id missing");
        let client_secret = env::var("TWITCH_CLIENT_SECRET").expect("TWITCH_CLIENT_SECRET missing");

        let credentials = RefreshingLoginCredentials::new(
            login,
            client_id,
            client_secret,
            self.command_handler.clone(),
        );

        let config = ClientConfig::new_simple(credentials);

        tracing::info!("Connected to Twitch");

        let (mut incoming_messages, client) = TwitchIRCClient::new(config);

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

#[async_trait]
impl TokenStorage for CommandHandler {
    type LoadError = DatabaseError;
    type UpdateError = DatabaseError;

    async fn load_token(&mut self) -> Result<UserAccessToken, Self::LoadError> {
        let access_token = self.db.get_auth("twitch_access_token")?.unwrap_or_default();
        let refresh_token = self
            .db
            .get_auth("twitch_refresh_token")?
            .unwrap_or_default();

        let created_at = DateTime::from_utc(
            DateTime::parse_from_rfc3339(
                &self.db.get_auth("twitch_created_at")?.unwrap_or_default(),
            )
            .expect("Failed to parse time")
            .naive_utc(),
            Utc,
        );

        let expires_at = match self.db.get_auth("twitch_expires_at")? {
            Some(date) => Some(DateTime::from_utc(
                DateTime::parse_from_rfc3339(&date)
                    .expect("Failed to parse time")
                    .naive_utc(),
                Utc,
            )),
            None => None,
        };

        Ok(UserAccessToken {
            access_token,
            refresh_token,
            created_at,
            expires_at,
        })
    }

    async fn update_token(&mut self, token: &UserAccessToken) -> Result<(), Self::UpdateError> {
        tracing::info!("Refreshed Twitch token!");

        self.twitch_api
            .as_ref()
            .expect("Tried to update Twitch tokens but the API is not initialized")
            .set_bearer_token(&token.access_token);

        self.db.save_token(token)
    }
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
        Some(&self.channel_id)
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
            Some(channel) => ChannelIdentifier::TwitchChannelID(channel.to_owned()),
            None => ChannelIdentifier::Anonymous,
        }
    }

    fn get_user_identifier(&self) -> UserIdentifier {
        UserIdentifier::TwitchID(self.msg.get_sender().id.clone())
    }
}
