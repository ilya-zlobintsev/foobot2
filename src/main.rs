#[macro_use]
extern crate diesel;
extern crate dotenv;
extern crate serde;

mod command_handler;
mod database;
mod platform;

use std::env;

use command_handler::CommandHandler;
use database::Database;
use dotenv::dotenv;
use platform::ChatPlatform;

use platform::discord::Discord;
use platform::twitch::Twitch;
use tokio::task;

#[tokio::main]
async fn main() {
    if let Err(e) = dotenv() {
        println!("{:?}", e);
        println!(".env file missing, using env variables")
    }

    tracing_subscriber::fmt::init();

    let db = Database::connect(env::var("DATABASE_URL").expect("DATABASE_URL missing"))
        .expect("Failed to connect to DB");

    let command_handler = CommandHandler::init(db).await;

    let twitch_handle = {
        let command_handler = command_handler.clone();

        task::spawn(async move {
            match Twitch::init(command_handler.clone()).await {
                Ok(twitch) => twitch.run().await,
                Err(e) => {
                    tracing::info!("Error loading Twitch: {:?}", e);
                    return;
                }
            }
        })
    };

    let discord_handle = task::spawn(async move {
        match Discord::init(command_handler).await {
            Ok(discord) => discord.run().await,
            Err(e) => {
                tracing::info!("Error loading Discord: {:?}", e);
                return;
            }
        }
    });

    tokio::try_join!(twitch_handle, discord_handle).unwrap();
}
