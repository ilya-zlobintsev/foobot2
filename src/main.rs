#[macro_use]
extern crate diesel;
#[macro_use]
extern crate rocket;

mod command_handler;
mod database;
mod platform;
mod web;

use dotenv::dotenv;
use std::env;

use command_handler::CommandHandler;
use database::Database;

use platform::discord::Discord;
use platform::twitch::Twitch;
use platform::ChatPlatform;

use rocket::futures::future::join_all;

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

    match Twitch::init(command_handler.clone()).await {
        Ok(twitch) => {
            handles.push(twitch.clone().run().await);

            channels
                .iter()
                .filter_map(|channel| {
                    if channel.platform == "twitch" {
                        Some(channel.channel.clone())
                    } else {
                        None
                    }
                })
                .for_each(|channel| {
                    twitch.join_channel(channel);
                });
        }
        Err(e) => {
            tracing::error!("Error loading Twitch: {:?}", e);
        }
    }

    match Discord::init(command_handler.clone()).await {
        Ok(discord) => {
            handles.push(discord.run().await);

            // command_handler
            //     .platform_senders
            //     .lock()
            //     .unwrap()
            //     .insert(ChatPlatformKind::Discord, discord.create_listener().await);
        }
        Err(e) => {
            tracing::error!("Error loading Discord: {:?}", e);
        }
    };

    join_all(handles).await;
}
