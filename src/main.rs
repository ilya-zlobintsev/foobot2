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
use platform::irc::Irc;
use platform::twitch::Twitch;
use platform::ChatPlatform;

#[tokio::main]
async fn main() {
    dotenv().unwrap_or_default();

    tracing_subscriber::fmt::init();

    let db = Database::connect(env::var("DATABASE_URL").expect("DATABASE_URL missing"))
        .expect("Failed to connect to DB");

    db.start_cron();

    let command_handler = CommandHandler::init(db).await;

    match &command_handler.twitch_api {
        Some(_) => match Twitch::init(command_handler.clone()).await {
            Ok(twitch) => twitch.run().await,
            Err(e) => tracing::warn!("Error loading Twitch: {:?}", e),
        },
        None => {
            tracing::info!("Twitch API not initialized! Not connecting to chat.");
        }
    }

    match Discord::init(command_handler.clone()).await {
        Ok(discord) => discord.run().await,
        Err(e) => {
            tracing::error!("Error loading Discord: {:?}", e);
        }
    };

    match Irc::init(command_handler.clone()).await {
        Ok(irc) => irc.run().await,
        Err(e) => {
            tracing::error!("Error loading IRC: {:?}", e);
        }
    }

    web::run(command_handler.clone()).await;
}
