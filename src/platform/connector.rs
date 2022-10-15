use super::*;
use crate::command_handler::CommandHandler;
use anyhow::{anyhow, Context};
use async_nats::Client;
use connector_schema::{
    IncomingMessage, OutgoingMessage, PermissionsRequest, PermissionsResponse,
    INCOMING_SUBJECT_PREFIX, OUTGOING_SUBJECT_PREFIX, PERMISSIONS_SUBJECT_PREFIX,
};
use futures::StreamExt;
use tracing::error;

pub struct ConnectorPlatform {
    command_handler: CommandHandler,
}

#[async_trait]
impl ChatPlatform for ConnectorPlatform {
    async fn init(command_handler: CommandHandler) -> Result<Box<Self>, ChatPlatformError> {
        Ok(Box::new(Self { command_handler }))
    }

    async fn run(self) {
        let incoming_subject = format!("{INCOMING_SUBJECT_PREFIX}*");
        let mut subscriber = self
            .command_handler
            .nats_client
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
                                nats_client: &self.command_handler.nats_client,
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
                                    .command_handler
                                    .nats_client
                                    .publish(outgoing_subject, outgoing_message.into())
                                    .await
                                {
                                    error!("Could not publish response: {err}");
                                }

                                let _ = self.command_handler.nats_client.flush().await;
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
        get_connector_permissions(
            self.nats_client,
            self.platform,
            self.msg.channel_id.clone(),
            self.msg.sender.id.clone(),
        )
        .await
        .expect("Could not get permissions")
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

pub async fn get_connector_permissions(
    nats_client: &Client,
    platform: &str,
    channel_id: String,
    user_id: String,
) -> anyhow::Result<Permissions> {
    let permissions_request = PermissionsRequest {
        channel_id,
        user_id,
    };
    let subject = format!("{PERMISSIONS_SUBJECT_PREFIX}{platform}");

    let message = nats_client
        .request(subject, permissions_request.into())
        .await
        .map_err(|err| anyhow!("NATS request error: {err}"))?;

    let permissions_response: PermissionsResponse = serde_json::from_slice(&message.payload)
        .context("Could not deserialize response payload")?;
    permissions_response.map_err(|err| anyhow!("{err}"))
}
