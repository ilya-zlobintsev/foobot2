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

use rocket::futures::future::join_all;

#[tokio::main]
async fn main() {
    dotenv().unwrap_or_default();

    tracing_subscriber::fmt::init();

    let db = Database::connect(env::var("DATABASE_URL").expect("DATABASE_URL missing"))
        .expect("Failed to connect to DB");

    db.start_cron();

    let command_handler = CommandHandler::init(db).await;

    let mut handles = Vec::new();

    let web_handle = web::run(command_handler.clone()).await;

    handles.push(web_handle);

    match Twitch::init(command_handler.clone()).await {
        Ok(twitch) => {
            handles.push(twitch.clone().run().await);
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

    match Irc::init(command_handler.clone()).await {
        Ok(irc) => handles.push(irc.run().await),
        Err(e) => {
            tracing::error!("Error loading IRC: {:?}", e);
        }
    }

    join_all(handles).await;
}
