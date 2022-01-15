use std::{collections::HashMap, env};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::web;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResponse {
    #[serde(rename = "client_id")]
    pub client_id: String,
    pub login: String,
    pub scopes: Vec<String>,
    #[serde(rename = "user_id")]
    pub user_id: String,
    #[serde(rename = "expires_in")]
    pub expires_in: i64,
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsersResponse {
    pub data: Vec<User>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub login: String,
    #[serde(rename = "display_name")]
    pub display_name: String,
    #[serde(rename = "type")]
    pub type_field: String,
    #[serde(rename = "broadcaster_type")]
    pub broadcaster_type: String,
    pub description: String,
    #[serde(rename = "profile_image_url")]
    pub profile_image_url: String,
    #[serde(rename = "offline_image_url")]
    pub offline_image_url: String,
    #[serde(rename = "view_count")]
    pub view_count: i64,
    #[serde(rename = "created_at")]
    pub created_at: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IvrModInfo {
    pub mods: Vec<Mod>,
    pub vips: Vec<Vip>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mod {
    pub id: String,
    pub login: String,
    pub display_name: String,
    pub granted_at: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vip {
    pub id: String,
    pub login: String,
    pub display_name: String,
    pub granted_at: String,
}

pub enum EventsubSubscriptionType {
    /// `broadcaster_id`
    ChannelFollow(String),
}

impl EventsubSubscriptionType {
    fn get_type(&self) -> &str {
        match self {
            Self::ChannelFollow(_) => "channel.follow",
        }
    }

    fn get_version(&self) -> &str {
        match self {
            EventsubSubscriptionType::ChannelFollow(_) => "1",
        }
    }

    fn get_condition(&self) -> Value {
        let mut condition = HashMap::new();

        match self {
            EventsubSubscriptionType::ChannelFollow(broadcaster_id) => {
                condition.insert("broadcaster_user_id", broadcaster_id.as_str());
            }
        }

        serde_json::to_value(condition).expect("Invalid condition")
    }

    fn get_transport() -> Value {
        let callback_url = format!("{}/hooks/twitch/eventsub", web::get_base_url(),);

        json!({
           "method": "webhook",
           "callback": callback_url,
           "secret": rocket::Config::SECRET_KEY,
        })
    }

    pub fn build_body(&self) -> String {
        let body = json!({
            "type": self.get_type(),
            "version": self.get_version(),
            "condition": self.get_condition(),
            "transport": Self::get_transport()
        });

        serde_json::to_string(&body).expect("Invalid eventsub body")
    }
}

#[derive(Debug)]
pub enum EventsubMessageType {
    WebhookCallbackVerification,
    Notification,
}

impl EventsubMessageType {
    pub fn from_str(s: &str) -> Self {
        match s {
            "webhook_callback_verification" => Self::WebhookCallbackVerification,
            "notification" => Self::Notification,
            _ => unimplemented!("Unknown message type {}", s),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsubMessage {
    pub challenge: Option<String>,
    pub subscription: EventsubSubscription,
    event: Option<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsubSubscription {
    pub id: String,
    pub status: String,
    #[serde(rename = "type")]
    pub sub_type: String,
    pub version: String,
    pub cost: i64,
    pub condition: serde_json::Value,
    #[serde(rename = "created_at")]
    pub created_at: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsubSubscriptionList {
    pub data: Vec<EventsubSubscription>,
}
