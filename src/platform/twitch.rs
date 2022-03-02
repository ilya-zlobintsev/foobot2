use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
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

#[derive(Clone)]
pub struct Twitch {
    client: Arc<Mutex<Option<TwitchClient>>>,
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
            client: Arc::new(Mutex::new(None)),
            command_handler,
            possible_prefixes,
            last_messages: Arc::new(DashMap::new()),
        }))
    }

    async fn run(self) {
        let platform_handler = self.command_handler.platform_handler.read().await;
        let twitch_api = platform_handler
            .twitch_api
            .as_ref()
            .expect("Twitch API is not initialized");

        let config = ClientConfig::new_simple(twitch_api.helix_api.credentials.clone());

        let (mut incoming_messages, client) = TwitchIRCClient::new(config);

        tracing::info!("Connected to Twitch");

        *self.client.lock().await = Some(client.clone());

        *twitch_api.chat_client.lock().await = Some(client.clone());

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
                for channel in twitch_channels {
                    tracing::info!("Joining channel {}", channel.login);
                    client.join(channel.login);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to fetch channels {:?}", e);
            }
        }
        drop(platform_handler);

        tokio::spawn(async move {
            while let Some(message) = incoming_messages.recv().await {
                match message {
                    ServerMessage::Privmsg(pm) => self.handle_message(pm).await,
                    ServerMessage::Whisper(whisper) => self.handle_message(whisper).await,
                    _ => (),
                }
            }
        });
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
    async fn handle_message<T: 'static + TwitchMessage + Send + Sync + Clone>(&self, msg: T) {
        if msg.get_sender().id == "82008718" && msg.get_content() == "pajaS 🚨 ALERT" {
            let client = self.client.lock().await;

            let client = client.as_ref().unwrap();

            client
                .privmsg("pajlada".to_owned(), "FeelsWeirdMan 👉 🚨".to_owned())
                .await
                .unwrap();
        }

        let Self {
            command_handler,
            last_messages,
            possible_prefixes,
            client,
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
                .await;

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

                let client_guard = client.lock().await;

                let client = client_guard.as_ref().unwrap();

                tracing::info!("Replying with {}", response);

                const MSG_LENGTH_LIMIT: usize = 420;

                if response.len() > MSG_LENGTH_LIMIT {
                    let response_bytes = response.into_bytes();

                    let mut chunks = response_bytes
                        .chunks(MSG_LENGTH_LIMIT)
                        .map(std::str::from_utf8)
                        .collect::<Result<Vec<&str>, _>>()
                        .unwrap()
                        .into_iter();

                    if let Some(pm) = msg.get_privmsg() {
                        client
                            .reply_to_privmsg(chunks.next().unwrap().to_owned(), pm)
                            .await
                            .expect("Failed to reply");

                        for chunk in chunks {
                            client
                                .say(pm.channel_login.clone(), chunk.to_owned())
                                .await
                                .expect("Failed to say");

                            sleep(Duration::from_secs(1)).await; // rate limiting
                        }
                    } else {
                        unimplemented!() // i don't bother with the whispers functionality because it doesn't properly work on twitch
                    }
                } else if let Some(pm) = msg.get_privmsg() {
                    client
                        .reply_to_privmsg(response, pm)
                        .await
                        .expect("Failed to reply");
                } else {
                    client
                        .privmsg(
                            channel.to_string(),
                            format!("/w {} {}", msg.get_sender().login, response),
                        )
                        .await
                        .expect("Failed to say");
                }

                sleep(Duration::from_secs(1)).await; // This is needed to adhere to the twitch rate limits
            }
        });
    }
}
