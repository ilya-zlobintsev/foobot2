use serde::Deserialize;

#[derive(Debug)]
pub enum EventSubEventType {
    ChannelUpdate(ChannelUpdateEvent),
    StreamOnline(StreamOnlineEvent),
    ChannelPointsCustomRewardRedemptionAdd(ChannelPointsCustomRewardRedemptionAddEvent),
}

#[derive(Debug, Deserialize)]
pub struct ChannelUpdateEvent {
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub title: String,
    pub language: String,
    pub category_id: String,
    pub category_name: String,
    pub is_mature: bool,
}

#[derive(Debug, Deserialize)]
pub struct StreamOnlineEvent {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    #[serde(rename = "type")]
    pub stream_type: String,
    pub started_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ChannelPointsCustomRewardRedemptionAddEvent {
    pub id: String,
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub user_input: String,
    pub status: String,
    pub reward: Reward,
    pub redeemed_at: String,
}

#[derive(Debug, Deserialize)]
pub struct Reward {
    pub id: String,
    pub title: String,
    pub cost: i64,
    pub prompt: String,
}
