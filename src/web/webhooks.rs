use std::str::FromStr;

use hmac::{Hmac, Mac, NewMac};
use rocket::{data::ToByteUnit, http::Status, outcome::Outcome, request::FromRequest, Data, State};
use serde_json::Value;
use sha2::Sha256;

use crate::command_handler::{
    twitch_api::eventsub::{
        EventSubNotification, EventSubNotificationType, EventSubVerficationCallback,
    },
    CommandHandler,
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

                    let twitch_api = cmd.twitch_api.as_ref().unwrap();

                    let client = twitch_api
                        .chat_client
                        .lock()
                        .await
                        .as_ref()
                        .unwrap()
                        .clone();

                    client
                        .say(
                            "boring_nick".to_string(),
                            format!("{:?}", notification.clone().get_event()),
                        )
                        .await.unwrap();

                    tracing::info!(
                        "Received EventSub notification: {:?}",
                        notification.get_event()
                    );

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
        })
    }
}
