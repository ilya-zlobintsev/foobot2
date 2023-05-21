use async_trait::async_trait;
use axum::{
    body::Bytes,
    extract::{FromRequestParts, State},
    routing::post,
    Router,
};
use hmac::{Hmac, Mac};
use http::{request::Parts, StatusCode};
use sha2::Sha256;
use std::str::FromStr;
use tokio::task;

use crate::{
    command_handler::twitch_api::eventsub::{events::*, *},
    platform::{ChannelIdentifier, ServerPlatformContext, UserIdentifier},
};

use super::state::AppState;

pub async fn eventsub_callback(
    properties: TwitchEventsubCallbackProperties,
    state: State<AppState>,
    body: Bytes,
) -> Result<String, StatusCode> {
    tracing::info!("Handling eventsub callback {:?}", properties.message_type);

    let message = serde_json::from_slice(&body).unwrap();

    let secret_key = &state.raw_secret_key;

    if properties.message_retry > 1 {
        tracing::warn!("Received EventSub message retry");
    }

    match verify_twitch_signature(&properties, &body, secret_key.as_bytes()).await {
        true => Ok({
            tracing::info!("Request signature verified");

            tracing::info!(
                "Handling EventSub notification {}",
                properties.subscription_type
            );

            match properties.message_type {
                EventSubNotificationType::Notification => {
                    let notification: EventSubNotification =
                        serde_json::from_value(message).expect("Invalid message format");

                    let cmd = state.cmd.clone();

                    task::spawn(async move {
                        let platform_handler = cmd.platform_handler.read().await;
                        let twitch_api = &platform_handler.twitch_api.as_ref().unwrap().helix_api;

                        if let Some(redeem) = cmd
                            .db
                            .get_eventsub_redeem(&notification.subscription.id)
                            .expect("DB error")
                        {
                            let event = notification
                                .get_event()
                                .expect("Failed to get notification event");

                            tracing::info!("Received EventSub notification: {:?}", event);

                            let broadcaster_id = event.get_broadcaster_id();

                            let (user_id, arguments) = match event {
                                EventSubEventType::ChannelUpdate(_)
                                | EventSubEventType::StreamOnline(_) => {
                                    (broadcaster_id.clone(), String::new())
                                }
                                EventSubEventType::ChannelPointsCustomRewardRedemptionAdd(
                                    event,
                                ) => (event.user_id, event.user_input),
                            };

                            let user = twitch_api
                                .get_user_by_id(&user_id)
                                .await
                                .expect("Failed to get user");

                            let context = ServerPlatformContext {
                                target_channel: ChannelIdentifier::TwitchChannel((
                                    broadcaster_id.to_string(),
                                    None,
                                )),
                                executing_user: UserIdentifier::TwitchID(user_id),
                                cmd: cmd.clone(),
                                display_name: user.display_name,
                            };

                            cmd.handle_server_message(
                                redeem.action,
                                redeem.mode,
                                context,
                                arguments
                                    .split_whitespace()
                                    .map(|s| s.to_string())
                                    .collect(),
                            )
                            .await
                            .expect("Failed to handle event");
                        } else {
                            tracing::warn!("Unregistered EventSub notification (no cleanup?)");
                        }
                    });

                    String::new()
                }
                EventSubNotificationType::WebhookCallbackVerification => {
                    let callback: EventSubVerficationCallback =
                        serde_json::from_value(message).expect("Invalid message format");

                    callback.challenge
                }
                EventSubNotificationType::Revocation => todo!(),
            }
        }),
        false => {
            tracing::warn!("REQUEST FORGERY DETECTED");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

async fn verify_twitch_signature(
    properties: &TwitchEventsubCallbackProperties,
    body: &[u8],
    secret_key: &[u8],
) -> bool {
    let mut hmac_message = Vec::new();

    hmac_message.extend_from_slice(properties.message_id.as_bytes());
    hmac_message.extend_from_slice(properties.message_timestamp.as_bytes());
    hmac_message.extend_from_slice(body);

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret_key).unwrap();

    mac.update(&hmac_message);

    let result = mac.finalize();

    let result_bytes = result.into_bytes();

    let hmac_signature = hex::encode(result_bytes);

    let expected_signature = properties
        .message_signature
        .strip_prefix("sha256=")
        .unwrap();

    hmac_signature == expected_signature
}

#[derive(Debug)]
pub struct TwitchEventsubCallbackProperties {
    message_id: String,
    message_retry: u32,
    message_type: EventSubNotificationType,
    message_signature: String,
    message_timestamp: String,
    subscription_type: String,
}

#[async_trait]
impl FromRequestParts<AppState> for TwitchEventsubCallbackProperties {
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, _: &AppState) -> Result<Self, Self::Rejection> {
        let headers = &parts.headers;

        Ok(Self {
            message_id: headers
                .get("Twitch-Eventsub-Message-Id")
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned(),
            message_retry: headers
                .get("Twitch-Eventsub-Message-Retry")
                .unwrap()
                .to_str()
                .unwrap()
                .parse()
                .unwrap(),
            message_type: EventSubNotificationType::from_str(
                headers
                    .get("Twitch-Eventsub-Message-Type")
                    .unwrap()
                    .to_str()
                    .unwrap(),
            )
            .expect("Invalid message type!"),
            message_signature: headers
                .get("Twitch-Eventsub-Message-Signature")
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned(),
            message_timestamp: headers
                .get("Twitch-Eventsub-Message-Timestamp")
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned(),
            subscription_type: headers
                .get("Twitch-Eventsub-Subscription-Type")
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned(),
        })
    }
}

pub fn create_router() -> Router<AppState> {
    Router::new().route("/twitch/eventsub", post(eventsub_callback))
}
