use super::*;
use crate::command_handler::CommandHandler;
use async_nats::Client;
use connector_schema::{
    IncomingMessage, OutgoingMessage, PermissionsRequest, PermissionsResponse,
    INCOMING_SUBJECT_PREFIX, OUTGOING_SUBJECT_PREFIX, PERMISSIONS_SUBJECT_PREFIX,
};
use futures::StreamExt;
use std::env;
use tracing::error;

pub struct ConnectorPlatform {
    client: Client,
    command_handler: CommandHandler,
}

#[async_trait]
impl ChatPlatform for ConnectorPlatform {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, ChatPlatformError> {
        let nats_addr = env::var("NATS_ADDRESS")?;
        let client = async_nats::connect(nats_addr).await.map_err(|err| {
            ChatPlatformError::ServiceError(format!("Could not connect to nats: {err}"))
        })?;

        Ok(Box::new(Self {
            client,
            command_handler,
        }))
    }

    async fn run(self) {
        let incoming_subject = format!("{INCOMING_SUBJECT_PREFIX}*");
        let mut subscriber = self
            .client
            .queue_subscribe(incoming_subject, "foobot_core".into())
            .await
            .expect("Failed to subscribe to incoming subject");

        tokio::spawn(async move {
            info!("Listening to connector messages");
            while let Some(msg) = subscriber.next().await {
                match IncomingMessage::try_from(msg.payload.as_ref()) {
                    Ok(incoming_message) => {
                        debug!("Got message: {incoming_message:?}");
                        if let Some(platform) = msg.subject.strip_prefix(INCOMING_SUBJECT_PREFIX) {
                            let platform_ctx = ConnectorPlatformContext {
                                nats_client: &self.client,
                                platform,
                                msg: &incoming_message,
                            };

                            if let Some(content) = self
                                .command_handler
                                .handle_message(&incoming_message.content, platform_ctx)
                                .await
                            {
                                let outgoing_message = OutgoingMessage {
                                    channel_id: incoming_message.channel_id,
                                    content,
                                    reply: incoming_message.id,
                                };
                                let outgoing_subject =
                                    format!("{OUTGOING_SUBJECT_PREFIX}{platform}");

                                if let Err(err) = self
                                    .client
                                    .publish(outgoing_subject, outgoing_message.into())
                                    .await
                                {
                                    error!("Could not publish response: {err}");
                                }

                                let _ = self.client.flush().await;
                            }
                        } else {
                            error!("Received incoming message on an unexpected subject: {msg:?}");
                        }
                    }
                    Err(err) => {
                        error!("Received malformed incoming message: {err}");
                    }
                }
            }
        });
    }
}

pub struct ConnectorPlatformContext<'a> {
    nats_client: &'a Client,
    platform: &'a str,
    msg: &'a IncomingMessage,
}

#[async_trait]
impl PlatformContext for ConnectorPlatformContext<'_> {
    async fn get_permissions_internal(&self) -> Permissions {
        let permissions_request = PermissionsRequest {
            channel_id: self.msg.channel_id.to_owned(),
            user_id: self.msg.sender.id.clone(),
        };
        let subject = format!(
            "{PERMISSIONS_SUBJECT_PREFIX}{platform}",
            platform = self.platform
        );

        // TODO error handling
        let message = self
            .nats_client
            .request(subject, permissions_request.into())
            .await
            .expect("Failed to fetch permissions");

        match PermissionsResponse::try_from(message.payload.as_ref())
            .expect("Could not deserialize permissions response")
        {
            PermissionsResponse::Ok(permissions) => permissions,
            PermissionsResponse::Error(err) => {
                panic!("Permissions request error: {err}")
            }
        }
    }

    fn get_channel(&self) -> ChannelIdentifier {
        ChannelIdentifier::new(self.platform, self.msg.channel_id.clone())
            .expect("Unable to construct channel identifier")
    }

    fn get_user_identifier(&self) -> UserIdentifier {
        UserIdentifier::from_string(&format!("{}:{}", self.platform, self.msg.sender.id))
            .expect("Unable to construct user identifier")
    }

    fn get_display_name(&self) -> &str {
        &self.msg.sender.display_name.as_deref().unwrap_or_default()
    }

    fn get_prefixes(&self) -> Vec<&str> {
        vec!["%"] // TODO
    }
}
