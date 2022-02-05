use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcasterIdCondition {
    pub broadcaster_user_id: String,
}

pub type ChannelUpdateCondition = BroadcasterIdCondition;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelPointsCustomRewardRedemptionAddCondition {
    pub broadcaster_user_id: String,
    pub reward_id: Option<String>,
}
