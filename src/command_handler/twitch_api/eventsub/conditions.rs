use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct BroadcasterIdCondition {
    pub broadcaster_user_id: String,
}

pub type ChannelUpdateCondition = BroadcasterIdCondition;
