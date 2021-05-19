#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

mod command_handler;
mod database;
mod platform;
mod web;

use std::env;

use command_handler::CommandHandler;
use database::Database;
use dotenv::dotenv;
use platform::ChatPlatform;

use platform::discord::Discord;
use platform::twitch::Twitch;
use rocket::futures::future::join_all;

use crate::platform::ChannelIdentifier;

#[tokio::main]
async fn main() {
    dotenv().unwrap_or_default();

    tracing_subscriber::fmt::init();

    let db = Database::connect(env::var("DATABASE_URL").expect("DATABASE_URL missing"))
        .expect("Failed to connect to DB");

    db.start_cron();

    let channels = db.get_channels().unwrap();

    let command_handler = CommandHandler::init(db).await;

    let mut handles = Vec::new();

    let web_handle = web::run(command_handler.clone()).await;

    handles.push(web_handle);

    let command_handler = command_handler.clone();

    match Twitch::init(command_handler.clone()).await {
        Ok(twitch) => {
            let handle = twitch.clone().run().await;

            channels
                .iter()
                .filter_map(|channel| {
                    if channel.platform
                        == ChannelIdentifier::TwitchChannelName("").get_platform_name()
                    {
                        Some(channel.channel.clone())
                    } else {
                        None
                    }
                })
                .for_each(|channel| {
                    twitch.join_channel(channel);
                });

            handles.push(handle);
        }
        Err(e) => {
            tracing::error!("Error loading Twitch: {:?}", e);
        }
    }

    match Discord::init(command_handler).await {
        Ok(discord) => {
            handles.push(discord.run().await);
        }
        Err(e) => {
            tracing::error!("Error loading Discord: {:?}", e);
        }
    };

    join_all(handles).await;
}
