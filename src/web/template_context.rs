use serde::Serialize;

use crate::database::models::{Channel, Command};

#[derive(Serialize)]
pub struct IndexContext {
    pub parent: &'static str,
    pub channel_amount: i64, 
}

#[derive(Serialize)]
pub struct ChannelsContext {
    pub parent: &'static str,
    pub channels: Vec<Channel>,
}

#[derive(Serialize)]
pub struct CommandsContext {
    pub parent: &'static str,
    pub channel: String,
    pub commands: Vec<Command>,
}
