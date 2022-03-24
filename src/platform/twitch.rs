use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::task;
use tokio::time::sleep;
use twitch_irc::login::{LoginCredentials, RefreshingLoginCredentials};
use twitch_irc::message::{Badge, PrivmsgMessage, ServerMessage, TwitchUserBasics, WhisperMessage};
use twitch_irc::{ClientConfig, SecureTCPTransport, TwitchIRCClient};

use crate::command_handler::CommandHandler;
use crate::database::Database;
use crate::platform::{ChannelIdentifier, ExecutionContext};

use super::{ChatPlatform, Permissions, UserIdentifier};

pub type Credentials = RefreshingLoginCredentials<Database>;
pub type TwitchClient = TwitchIRCClient<SecureTCPTransport, Credentials>;

pub const MSG_LENGTH_LIMIT: usize = 420;

#[derive(Clone)]
pub struct Twitch {
    command_handler: CommandHandler,
    possible_prefixes: Arc<[String; 5]>,
    last_messages: Arc<DashMap<String, String>>,
}

#[async_trait]
impl ChatPlatform for Twitch {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, super::ChatPlatformError> {
        let login = {
            let platform_handler = command_handler.platform_handler.read().await;
            let twitch_api = platform_handler
                .twitch_api
                .as_ref()
                .expect("Twitch API is not initialized");

            twitch_api
                .helix_api
                .credentials
                .get_credentials()
                .await
                .map_err(|_| super::ChatPlatformError::MissingAuthentication)?
                .login
        };

        let possible_prefixes = Arc::new([
            Self::get_prefix(),
            format!("{},", &login),
            format!("@{}", &login),
            format!("@{},", &login),
            login,
        ]);

        Ok(Box::new(Self {
            command_handler,
            possible_prefixes,
            last_messages: Arc::new(DashMap::new()),
        }))
    }

    async fn run(self) {
        let (tx, mut rx) = mpsc::unbounded_channel::<SenderMessage>();

        let platform_handler = self.command_handler.platform_handler.read().await;
        let twitch_api = platform_handler
            .twitch_api
            .as_ref()
            .expect("Twitch API is not initialized");

        let config = ClientConfig::new_simple(twitch_api.helix_api.credentials.clone());

        let (mut incoming_messages, client): (_, TwitchClient) = TwitchIRCClient::new(config);

        tracing::info!("Connected to Twitch");

        *twitch_api.chat_sender.lock().await = Some(tx.clone());

        let channels = self.command_handler.db.get_channels().unwrap();

        let channel_ids: Vec<&str> = channels
            .iter()
            .filter_map(|channel| {
                if channel.platform == "twitch" {
                    Some(channel.channel.as_str())
                } else {
                    None
                }
            })
            .collect();

        match twitch_api
            .helix_api
            .get_users(None, Some(&channel_ids))
            .await
        {
            Ok(twitch_channels) => {
                let mut channels = HashSet::new();

                for channel in twitch_channels {
                    channels.insert(channel.login);
                }

                client
                    .set_wanted_channels(channels)
                    .expect("Invalid channels");
            }
            Err(e) => {
                tracing::warn!("Failed to fetch channels {:?}", e);
            }
        }
        drop(platform_handler);

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                tracing::trace!("Received Twitch sender message: {:?}", msg);
                match msg {
                    SenderMessage::Privmsg(mut pm) => {
                        while pm.message.len() > MSG_LENGTH_LIMIT {
                            let mut rest = pm.message.split_off(MSG_LENGTH_LIMIT);

                            if pm.message.chars().last().map(|c| c.is_whitespace()) != Some(true) {
                                let mut words = pm.message.split_whitespace();
                                if let Some(last_word) = words.next_back() {
                                    rest = format!("{} {}", last_word, rest);
                                }
                            }

                            client
                                .say_in_response(
                                    pm.channel_login.clone(),
                                    pm.message,
                                    pm.reply_to_id.clone(),
                                )
                                .await
                                .expect("Failed to send message");

                            pm.message = rest;

                            sleep(Duration::from_secs(1)).await;
                        }
                        if let Err(e) = client
                            .say_in_response(pm.channel_login, pm.message, pm.reply_to_id)
                            .await
                        {
                            tracing::error!("Error sending twitch message: {}", e);
                        }
                        sleep(Duration::from_secs(1)).await;
                    }
                    SenderMessage::JoinChannel(channel_login) => {
                        if let Err(e) = client.join(channel_login) {
                            tracing::error!("Failed to join channel: {}", e);
                        }
                    }
                }
            }
        });

        tokio::spawn(async move {
            while let Some(message) = incoming_messages.recv().await {
                match message {
                    ServerMessage::Privmsg(pm) => self.handle_message(pm, tx.clone()).await,
                    ServerMessage::Whisper(whisper) => {
                        self.handle_message(whisper, tx.clone()).await
                    }
                    _ => (),
                }
            }
        });
    }
}

pub trait TwitchMessage {
    fn get_badges(&self) -> &Vec<Badge>;

    fn get_sender(&self) -> &TwitchUserBasics;

    fn get_channel(&self) -> Option<(&str, &str)>;

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

    fn get_channel(&self) -> Option<(&str, &str)> {
        Some((&self.channel_id, &self.channel_login))
    }

    fn get_content(&self) -> &str {
        &self.message_text
    }

    fn get_privmsg(&self) -> Option<&PrivmsgMessage> {
        Some(self)
    }
}

impl TwitchMessage for WhisperMessage {
    fn get_badges(&self) -> &Vec<Badge> {
        &self.badges
    }

    fn get_sender(&self) -> &TwitchUserBasics {
        &self.sender
    }

    fn get_channel(&self) -> Option<(&str, &str)> {
        None
    }

    fn get_content(&self) -> &str {
        &self.message_text
    }

    fn get_privmsg(&self) -> Option<&PrivmsgMessage> {
        None
    }
}

#[derive(Clone)]
pub struct TwitchExecutionContext<M: TwitchMessage + std::marker::Sync + Clone> {
    msg: M,
    prefixes: Vec<String>,
}

#[async_trait]
impl<T: TwitchMessage + std::marker::Sync + Clone> ExecutionContext for TwitchExecutionContext<T> {
    async fn get_permissions_internal(&self) -> Permissions {
        let mut badges_iter = self.msg.get_badges().iter();

        if badges_iter.any(|badge| (badge.name) == "broadcaster") {
            Permissions::ChannelOwner
        } else if badges_iter.any(|badge| badge.name == "moderator") {
            Permissions::ChannelMod
        } else {
            Permissions::Default
        }
    }

    fn get_channel(&self) -> ChannelIdentifier {
        match self.msg.get_channel() {
            Some((id, title)) => {
                ChannelIdentifier::TwitchChannel((id.to_string(), Some(title.to_string())))
            }
            None => ChannelIdentifier::Anonymous,
        }
    }

    fn get_user_identifier(&self) -> UserIdentifier {
        UserIdentifier::TwitchID(self.msg.get_sender().id.clone())
    }

    fn get_display_name(&self) -> &str {
        &self.msg.get_sender().name
    }

    fn get_prefixes(&self) -> Vec<&str> {
        self.prefixes.iter().map(|s| s.as_str()).collect()
    }
}

impl Twitch {
    async fn handle_message<T: 'static + TwitchMessage + Send + Sync + Clone>(
        &self,
        msg: T,
        tx: UnboundedSender<SenderMessage>,
    ) {
        let Self {
            command_handler,
            last_messages,
            possible_prefixes,
            ..
        } = self.clone();

        task::spawn(async move {
            let prefixes = if let Some(custom_prefix) = command_handler
                .db
                .get_prefix_in_channel(&ChannelIdentifier::TwitchChannel((
                    match msg.get_channel() {
                        Some((channel_id, _)) => channel_id.to_string(),
                        None => msg.get_sender().id.clone(),
                    },
                    None,
                )))
                .expect("DB error")
            {
                vec![custom_prefix]
            } else {
                possible_prefixes.to_vec()
            };

            let context = TwitchExecutionContext {
                msg: msg.clone(),
                prefixes,
            };

            let recieved_instant = Instant::now();

            tracing::debug!("Recieved a command at {:?}", recieved_instant);

            let response = command_handler
                .handle_message(msg.get_content(), context)
                .await
                .map(|s| s.replace('\n', " "));

            tracing::debug!(
                "Command took {}ms to process",
                recieved_instant.elapsed().as_millis()
            );

            if let Some(mut response) = response {
                if response.trim().is_empty() {
                    tracing::info!("Empty command response");
                    return;
                }

                let channel = match msg.get_channel() {
                    Some((channel_id, _)) => channel_id,
                    None => &msg.get_sender().id,
                };

                if let Some(last_msg) = last_messages.get(channel) {
                    if *last_msg == response {
                        tracing::info!("Detected same matching message, adding an empty character");

                        let magic_char = char::from_u32(0x000E0000).unwrap();

                        if response.ends_with(magic_char) {
                            response.remove(response.len() - 1);
                        } else {
                            response.push(' ');
                            response.push(magic_char);
                        }
                    }
                }

                last_messages.insert(channel.to_string(), response.clone());

                tracing::info!("Replying with {}", response);

                if let Some(pm) = msg.get_privmsg() {
                    tx.send(SenderMessage::Privmsg(Privmsg {
                        channel_login: pm.channel_login.clone(),
                        message: response,
                        reply_to_id: Some(pm.message_id.clone()),
                    }))
                    .unwrap();
                } else {
                    tx.send(SenderMessage::Privmsg(Privmsg {
                        channel_login: msg.get_sender().login.clone(),
                        message: format!("/w {} {}", msg.get_sender().login, response),
                        reply_to_id: None,
                    }))
                    .unwrap();
                }

                sleep(Duration::from_secs(1)).await; // This is needed to adhere to the twitch rate limits
            }
        });
    }
}

#[derive(Clone, Debug)]
pub enum SenderMessage {
    Privmsg(Privmsg),
    JoinChannel(String),
}

#[derive(Clone, Debug)]
pub struct Privmsg {
    pub channel_login: String,
    pub message: String,
    pub reply_to_id: Option<String>,
}
