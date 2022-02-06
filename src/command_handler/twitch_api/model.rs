use serde::{Deserialize, Serialize};

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

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomRewardsResponse {
    pub data: Vec<CustomReward>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomReward {
    pub broadcaster_name: String,
    pub broadcaster_login: String,
    pub broadcaster_id: String,
    pub id: String,
    pub background_color: String,
    pub is_enabled: bool,
    pub cost: i64,
    pub title: String,
    pub prompt: String,
    pub is_user_input_required: bool,
    pub max_per_stream_setting: MaxPerStreamSetting,
    pub max_per_user_per_stream_setting: MaxPerUserPerStreamSetting,
    pub global_cooldown_setting: GlobalCooldownSetting,
    pub is_paused: bool,
    pub is_in_stock: bool,
    pub default_image: DefaultImage,
    pub should_redemptions_skip_request_queue: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaxPerStreamSetting {
    pub is_enabled: bool,
    pub max_per_stream: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaxPerUserPerStreamSetting {
    pub is_enabled: bool,
    pub max_per_user_per_stream: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobalCooldownSetting {
    pub is_enabled: bool,
    pub global_cooldown_seconds: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DefaultImage {
    pub url_1x: String,
    pub url_2x: String,
    pub url_4x: String,
}
