#[macro_use]
extern crate diesel;
#[macro_use]
extern crate rocket;

mod command_handler;
mod database;
mod platform;
mod web;

use command_handler::{get_admin_channel, CommandHandler};
use database::Database;
use dotenv::dotenv;
use platform::local::Local;
use std::env;

use platform::discord::Discord;
use platform::irc::Irc;
use platform::twitch::Twitch;
use platform::ChatPlatform;
use platform::telegram::Telegram;

#[tokio::main]
async fn main() {
    dotenv().unwrap_or_default();

    tracing_subscriber::fmt::init();

    let db = Database::connect(env::var("DATABASE_URL").expect("DATABASE_URL missing"))
        .expect("Failed to connect to DB");

    db.start_cron();

    let command_handler = CommandHandler::init(db).await;

    match &command_handler.platform_handler.read().await.twitch_api {
        Some(_) => match Twitch::init(command_handler.clone()).await {
            Ok(twitch) => twitch.run().await,
            Err(e) => tracing::warn!("Platform {:?}", e),
        },
        None => {
            tracing::info!("Twitch is not initialized! Not connecting to chat.");
        }
    }

    match Discord::init(command_handler.clone()).await {
        Ok(discord) => discord.run().await,
        Err(e) => {
            tracing::warn!("Error loading Discord: {:?}", e);
        }
    };

    match Irc::init(command_handler.clone()).await {
        Ok(irc) => irc.run().await,
        Err(e) => {
            tracing::warn!("Error loading IRC: {:?}", e);
        }
    }

    match Telegram::init(command_handler.clone()).await {
        Ok(telegram) => telegram.run().await,
        Err(e) => tracing::warn!("Error loading Telegram: {:?}", e),
    }

    match Local::init(command_handler.clone()).await {
        Ok(local) => local.run().await,
        Err(e) => tracing::warn!("Failed to initialize the local platform: {:?}", e),
    }

    if let Some(admin_channel) = get_admin_channel() {
        let platform_handler = command_handler.platform_handler.read().await;
        if let Err(e) = platform_handler
            .send_to_channel(
                admin_channel,
                format!("Foobot2 {} up and running", get_version()),
            )
            .await
        {
            tracing::warn!("Failed to send startup message: {}", e);
        }
    }

    web::run(command_handler.clone()).await;
}

pub fn get_version() -> String {
    format!(
        "{}, commit {} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("GIT_HASH"),
        env!("PROFILE")
    )
}
