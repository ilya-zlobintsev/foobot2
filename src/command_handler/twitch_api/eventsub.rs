pub mod conditions;
pub mod events;

use std::str::FromStr;

use anyhow::anyhow;
use serde::Deserialize;
use serde::Serialize;
use serde_json::{json, Value};

use crate::web;

use self::conditions::*;
use self::events::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventSubSubscriptionType {
    ChannelUpdate(ChannelUpdateCondition),
    ChannelPointsCustomRewardRedemptionAdd(ChannelPointsCustomRewardRedemptionAddCondition),
}

impl EventSubSubscriptionType {
    pub fn get_type(&self) -> &str {
        match self {
            Self::ChannelUpdate(_) => "channel.update",
            Self::ChannelPointsCustomRewardRedemptionAdd(_) => {
                "channel.channel_points_custom_reward_redemption.add"
            }
        }
    }

    fn get_version(&self) -> &str {
        match self {
            _ => "1",
        }
    }

    pub fn get_condition(&self) -> Value {
        match self {
            Self::ChannelUpdate(condition) => serde_json::to_value(condition).unwrap(),
            Self::ChannelPointsCustomRewardRedemptionAdd(condition) => {
                serde_json::to_value(condition).unwrap()
            }
        }
    }

    fn get_transport() -> Value {
        let callback_url = format!("{}/hooks/twitch/eventsub", web::get_base_url(),);

        json!({
           "method": "webhook",
           "callback": callback_url,
           "secret": rocket::Config::SECRET_KEY,
        })
    }

    pub fn build_body(&self) -> Value {
        json!({
            "type": self.get_type(),
            "version": self.get_version(),
            "condition": self.get_condition(),
            "transport": Self::get_transport()
        })
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum EventSubNotificationType {
    Notification,
    WebhookCallbackVerification,
    Revocation,
}

impl FromStr for EventSubNotificationType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "notification" => Ok(Self::Notification),
            "webhook_callback_verification" => Ok(Self::WebhookCallbackVerification),
            "revocation" => Ok(Self::Revocation),
            _ => Err(anyhow!("Invalid notification type")),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventSubNotification {
    pub subscription: EventSubSubscription,
    event: Value,
}

impl EventSubNotification {
    pub fn get_event(self) -> anyhow::Result<EventSubEventType> {
        Ok(match self.subscription.sub_type.as_str() {
            "channel.update" => {
                EventSubEventType::ChannelUpdate(serde_json::from_value(self.event)?)
            }
            "channel.channel_points_custom_reward_redemption.add" => {
                EventSubEventType::ChannelPointsCustomRewardRedemptionAdd(serde_json::from_value(
                    self.event,
                )?)
            }
            _ => unimplemented!(),
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventSubSubscription {
    pub id: String,
    pub status: String,
    #[serde(rename = "type")]
    pub sub_type: String,
    pub version: String,
    pub cost: i64,
    pub condition: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct EventSubVerficationCallback {
    pub subscription: EventSubSubscription,
    pub challenge: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventSubSubscriptionResponse {
    pub data: Vec<EventSubSubscription>,
}
