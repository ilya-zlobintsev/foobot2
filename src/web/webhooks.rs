use std::str::FromStr;

use hmac::{Hmac, Mac};
use rocket::{data::ToByteUnit, http::Status, outcome::Outcome, request::FromRequest, Data, State};
use serde_json::Value;
use sha2::Sha256;
use tokio::task;

use crate::{
    command_handler::{
        twitch_api::eventsub::{events::*, *},
        CommandHandler,
    },
    platform::{ChannelIdentifier, ServerExecutionContext, UserIdentifier},
};

#[post("/twitch/eventsub", data = "<body>")]
pub async fn eventsub_callback(
    properties: TwitchEventsubCallbackProperties,
    cmd: &State<CommandHandler>,
    body: Data<'_>,
) -> Result<String, Status> {
    tracing::info!("Handling eventsub callback {:?}", properties.message_type);

    let body_stream = body.open(32i32.mebibytes());

    let body = body_stream.into_bytes().await.unwrap();

    let message: Value = serde_json::from_slice(&body).expect("Parse error");

    let secret_key = rocket::Config::SECRET_KEY;

    match verify_twitch_signature(&properties, &body, secret_key).await {
        true => Ok({
            tracing::info!("Request signature verified");

            match properties.message_type {
                EventSubNotificationType::Notification => {
                    let notification: EventSubNotification =
                        serde_json::from_value(message).expect("Invalid message format");

                    let event = notification
                        .get_event()
                        .expect("Failed to get notification event");

                    tracing::info!("Received EventSub notification: {:?}", event);

                    let cmd = (*cmd).clone();

                    task::spawn(async move {
                        let twitch_api = &cmd.twitch_api.as_ref().unwrap().helix_api;

                        let broadcaster_id = event.get_broadcaster_id();

                        if let Some(action) = cmd
                            .db
                            .get_eventsub_redeem_action(
                                &broadcaster_id,
                                &properties.subscription_type,
                            )
                            .expect("DB error")
                        {
                            let user_id = match event {
                                EventSubEventType::ChannelUpdate(_)
                                | EventSubEventType::StreamOnline(_) => broadcaster_id.clone(),
                                EventSubEventType::ChannelPointsCustomRewardRedemptionAdd(
                                    event,
                                ) => event.user_id,
                            };

                            let user = twitch_api
                                .get_user_by_id(&user_id)
                                .await
                                .expect("Failed to get user");

                            let context = ServerExecutionContext {
                                target_channel: ChannelIdentifier::TwitchChannelID(
                                    broadcaster_id.to_string(),
                                ),
                                executing_user: UserIdentifier::TwitchID(user_id),
                                cmd: cmd.clone(),
                                display_name: user.display_name,
                            };

                            cmd.handle_server_message(action, context)
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
            Err(Status::Unauthorized)
        }
    }
}

async fn verify_twitch_signature(
    properties: &TwitchEventsubCallbackProperties,
    body: &[u8],
    secret_key: &str,
) -> bool {
    let mut hmac_message = Vec::new();

    hmac_message.extend_from_slice(properties.message_id.as_bytes());
    hmac_message.extend_from_slice(properties.message_timestamp.as_bytes());
    hmac_message.extend_from_slice(body);

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes()).unwrap();

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

#[rocket::async_trait]
impl<'r> FromRequest<'r> for TwitchEventsubCallbackProperties {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let headers = request.headers();

        Outcome::Success(Self {
            message_id: headers
                .get("Twitch-Eventsub-Message-Id")
                .next()
                .unwrap()
                .to_string(),
            message_retry: headers
                .get("Twitch-Eventsub-Message-Retry")
                .next()
                .unwrap()
                .parse()
                .unwrap(),
            message_type: EventSubNotificationType::from_str(
                &headers
                    .get("Twitch-Eventsub-Message-Type")
                    .next()
                    .unwrap()
                    .to_string(),
            )
            .expect("Invalid message type!"),
            message_signature: headers
                .get("Twitch-Eventsub-Message-Signature")
                .next()
                .unwrap()
                .to_string(),
            message_timestamp: headers
                .get("Twitch-Eventsub-Message-Timestamp")
                .next()
                .unwrap()
                .to_string(),
            subscription_type: headers
                .get("Twitch-Eventsub-Subscription-Type")
                .next()
                .unwrap()
                .to_string(),
        })
    }
}
