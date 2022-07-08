use super::{ChannelIdentifier, ChatPlatform, ChatPlatformError, ExecutionContext, UserIdentifier};
use crate::command_handler::CommandHandler;
use connector_schema::{IncomingMessage, OutgoingMessage};
use foobot_permissions_proto::channel_permissions_response::Permissions;
use foobot_permissions_proto::permissions_handler_client::PermissionsHandlerClient;
use foobot_permissions_proto::ChannelPermissionsRequest;
use futures_util::stream::StreamExt;
use redis::aio::{MultiplexedConnection, PubSub};
use redis::AsyncCommands;
use std::sync::Arc;
use std::{env, str::FromStr};
use tonic::transport::Channel;
use tracing::error;

pub struct Connector {
    command_handler: CommandHandler,
    pubsub_conn: PubSub,
    publish_conn: MultiplexedConnection,
    incoming_channel_prefix: Arc<String>,
    outgoing_channel_prefix: Arc<String>,
    permissions_handler_client: PermissionsHandlerClient<Channel>,
}

#[async_trait]
impl ChatPlatform for Connector {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, ChatPlatformError> {
        let pubsub_conn = command_handler
            .redis_client
            .get_async_connection()
            .await
            .expect("Failed to connect to redis")
            .into_pubsub();
        let publish_conn = command_handler.redis_conn.clone();

        let incoming_channel_prefix = Arc::new(
            env::var("INCOMING_MESSAGES_CHANNEL_PREFIX")
                .unwrap_or_else(|_| "messages.incoming.".to_owned()),
        );
        let outgoing_channel_prefix = Arc::new(
            env::var("OUTGOING_MESSAGES_CHANNEL_PREFIX")
                .unwrap_or_else(|_| "messages.outgoing.".to_owned()),
        );

        let permissions_handler_url = env::var("FOOBOT_PERMISSIONS_HANDLER_URL")
            .unwrap_or_else(|_| "http://localhost:50053".to_owned());
        let permissions_handler_client = PermissionsHandlerClient::connect(permissions_handler_url)
            .await
            .unwrap();

        Ok(Box::new(Self {
            command_handler,
            pubsub_conn,
            publish_conn,
            incoming_channel_prefix,
            outgoing_channel_prefix,
            permissions_handler_client,
        }))
    }

    async fn run(mut self) {
        tokio::spawn(async move {
            let channel_glob = format!("{}*", self.incoming_channel_prefix);
            info!("Subscribing to {channel_glob}");
            self.pubsub_conn.psubscribe(channel_glob).await.unwrap();

            let prefixes = vec![Connector::get_prefix()];

            info!("Listening on connector messages");
            while let Some(msg) = self.pubsub_conn.on_message().next().await {
                let command_handler = self.command_handler.clone();
                let mut publish_conn = self.publish_conn.clone();
                let prefixes = prefixes.clone();
                let incoming_channel_prefix = self.incoming_channel_prefix.clone();
                let outgoing_channel_prefix = self.outgoing_channel_prefix.clone();
                let permissions_handler_client = self.permissions_handler_client.clone();

                tokio::spawn(async move {
                    match serde_json::from_slice(msg.get_payload_bytes()) {
                        Ok(incoming_message) => {
                            info!("Received {incoming_message:?}");

                            let platform = msg
                                .get_channel_name()
                                .strip_prefix(&*incoming_channel_prefix)
                                .expect("Received message on unexpected redis channel");

                            let connector_message = ConnectorMessage {
                                incoming_message,
                                platform,
                                prefixes,
                                permissions_handler_client,
                            };

                            if let Some(response) = command_handler
                                .handle_message(
                                    &connector_message.incoming_message.contents,
                                    &connector_message,
                                )
                                .await
                            {
                                let outgoing_message = OutgoingMessage {
                                    channel_id: connector_message.incoming_message.channel.id,
                                    contents: response,
                                };

                                let outgoing_channel =
                                    format!("{}{}", outgoing_channel_prefix, platform);

                                info!("Repying with {outgoing_message:?} to {outgoing_channel}");

                                publish_conn
                                    .publish::<_, _, u64>(outgoing_channel, outgoing_message)
                                    .await
                                    .expect("Failed to publish");
                            }
                        }
                        Err(error) => {
                            error!("Received malformed incoming message: {error}")
                        }
                    }
                });
            }
        });
    }
}

pub struct ConnectorMessage<'a> {
    pub incoming_message: IncomingMessage<'a>,
    pub platform: &'a str,
    pub prefixes: Vec<String>,
    pub permissions_handler_client: PermissionsHandlerClient<Channel>,
}

#[async_trait]
impl<'a> ExecutionContext for &ConnectorMessage<'a> {
    async fn get_permissions_internal(&self) -> Permissions {
        let request = ChannelPermissionsRequest {
            platform: self.platform.to_owned(),
            channel_id: self.incoming_message.channel.id.to_owned(),
            user_id: self.incoming_message.sender.id.to_owned(),
        };

        let response = self
            .permissions_handler_client
            .clone()
            .get_permissions_in_channel(request)
            .await
            .expect("Failed to fetch permissions");

        response.into_inner().permissions()
    }

    fn get_channel(&self) -> ChannelIdentifier {
        ChannelIdentifier::from_str(&format!(
            "{}:{}",
            self.platform, self.incoming_message.channel.id
        ))
        .expect("Could not parse platform")
    }

    fn get_user_identifier(&self) -> UserIdentifier {
        UserIdentifier::from_string(&format!(
            "{}:{}",
            self.platform, self.incoming_message.sender.id,
        ))
        .expect("Could not parse user")
    }

    fn get_display_name(&self) -> &str {
        self.incoming_message.sender.display_name
    }

    fn get_prefixes(&self) -> Vec<&str> {
        self.prefixes.iter().map(|s| s.as_str()).collect()
    }
}
