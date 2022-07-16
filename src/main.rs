#[macro_use]
extern crate diesel;
#[macro_use]
extern crate rocket;

mod api;
mod command_handler;
mod database;
mod platform;
mod rpc;

use command_handler::{get_admin_channel, CommandHandler};
use database::Database;
use dotenv::dotenv;
use platform::connector::Connector;
use platform::ChatPlatform;
use std::env;

#[tokio::main]
async fn main() {
    dotenv().unwrap_or_default();

    tracing_subscriber::fmt::init();

    let db = Database::connect(env::var("DATABASE_URL").expect("DATABASE_URL missing"))
        .await
        .expect("Failed to connect to DB");

    db.start_cron();
    db.load_channels_into_redis().await.unwrap();

    let command_handler = CommandHandler::init(db).await;

    Connector::init(command_handler.clone())
        .await
        .expect("Failed to init connector")
        .run()
        .await;

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

    rpc::start_server(command_handler.clone());

    api::run(command_handler).await;
}

pub fn get_version() -> String {
    format!(
        "{}, commit {} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("GIT_HASH"),
        env!("PROFILE")
    )
}
