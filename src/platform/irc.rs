use std::sync::RwLock;
use std::time::Duration;
use std::{env, sync::Arc};

use crate::command_handler::CommandHandler;
use crate::platform::{PlatformContext, UserIdentifier};

use super::{ChannelIdentifier, ChatPlatform, ChatPlatformError, Permissions};
use futures::StreamExt;
use irc::client::{prelude::*, Client};
use tokio::task;

#[derive(Clone)]
pub struct Irc {
    client: Arc<RwLock<Client>>,
    command_prefix: Arc<String>,
    command_handler: CommandHandler,
}

impl Irc {
    async fn handle_message(&self, message: Message) {
        let Self {
            command_handler,
            client,
            command_prefix,
            ..
        } = self.clone();

        task::spawn(async move {
            if let Command::PRIVMSG(_, content) = &message.command {
                let context = IrcPlatformContext {
                    message: &message,
                    command_prefix,
                };
                if let Some(response) = command_handler.handle_message(content, context).await {
                    let client = client.read().unwrap();

                    client
                        .send_privmsg(message.response_target().unwrap(), response)
                        .expect("Failed to send PRIVMSG");
                }
            }
        });
    }
}

#[async_trait]
impl ChatPlatform for Irc {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, ChatPlatformError> {
        let command_prefix = Self::get_prefix();

        let config = Config {
            nickname: env::var("IRC_NICKNAME").ok(),
            nick_password: env::var("IRC_PASSWORD").ok(),
            server: env::var("IRC_SERVER").ok(),
            alt_nicks: vec!["foobot_alt_nick".to_owned()],
            channels: {
                match env::var("IRC_CHANNELS") {
                    Ok(channels) => {
                        dbg!(&channels);
                        channels.split(',').map(|s| s.to_owned()).collect()
                    }
                    Err(e) => {
                        tracing::info!("Failed to load IRC channels: {}", e);
                        vec![]
                    }
                }
            },
            ..Default::default()
        };

        tracing::info!("IRC Config: {:?}", config);

        let client = Client::from_config(config)
            .await
            .map_err(|_| ChatPlatformError::MissingAuthentication)?;
        {
            let mut platform_handler = command_handler.platform_handler.write().await;
            platform_handler.irc_sender = Some(client.sender());
            info!("Configured irc sender");
        }

        Ok(Box::new(Self {
            command_prefix: Arc::new(command_prefix),
            command_handler,
            client: Arc::new(RwLock::new(client)),
        }))
    }

    async fn run(self) {
        let client = self.client.clone();
        let get_stream = move || {
            let mut client = client.write().unwrap();

            client.identify().expect("Failed to identify");

            client.stream().unwrap()
        };
        let mut stream = get_stream();

        tracing::info!("IRC connected");

        task::spawn(async move {
            loop {
                match stream.next().await.transpose() {
                    Ok(Some(message)) => {
                        self.handle_message(message).await;
                    }
                    Ok(None) => (),
                    Err(e) => {
                        tracing::warn!("IRC error: {}", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        stream = get_stream();
                    }
                }
            }
        });
    }
}

#[derive(Clone)]
struct IrcPlatformContext<'a> {
    message: &'a Message,
    command_prefix: Arc<String>,
}

// TODO remove the unwraps
#[async_trait]
impl PlatformContext for IrcPlatformContext<'_> {
    async fn get_permissions_internal(&self) -> Permissions {
        // TODO
        Permissions::Default
    }

    fn get_channel(&self) -> ChannelIdentifier {
        ChannelIdentifier::IrcChannel(self.message.response_target().unwrap().to_owned())
    }

    fn get_user_identifier(&self) -> UserIdentifier {
        UserIdentifier::IrcName(self.message.source_nickname().unwrap().to_owned())
    }

    fn get_display_name(&self) -> &str {
        self.message.source_nickname().unwrap()
    }

    fn get_prefixes(&self) -> Vec<&str> {
        vec![&self.command_prefix]
    }
}
