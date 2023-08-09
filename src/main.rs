#[macro_use]
extern crate diesel;

mod api;
mod command_handler;
mod database;
mod platform;
mod rpc;

use command_handler::{get_admin_channel, CommandHandler};
use database::Database;
use dotenv::dotenv;
use opentelemetry::sdk::Resource;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use platform::local::Local;
use std::env;
use std::time::Duration;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{prelude::*, Registry};

use platform::connector::ConnectorPlatform;
use platform::discord::Discord;
use platform::irc::Irc;
use platform::twitch::Twitch;
use platform::ChatPlatform;

#[tokio::main]
async fn main() {
    dotenv().unwrap_or_default();
    init_tracing();

    let db = Database::connect(env::var("DATABASE_URL").expect("DATABASE_URL missing"))
        .expect("Failed to connect to DB");

    db.start_cron();

    let command_handler = CommandHandler::init(db).await;

    match ConnectorPlatform::init(command_handler.clone()).await {
        Ok(connector) => connector.run().await,
        Err(err) => {
            tracing::warn!("Could not initialize connector: {err:?}");
        }
    }

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

    command_handler::geohub::start_listener(
        command_handler.db.clone(),
        command_handler.platform_handler.clone(),
        Duration::from_secs(60),
        Default::default(),
    )
    .await
    .expect("Could not start GeoHub loop");

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

fn init_tracing() {
    let otlp_host = env::var("OTLP_HOST").expect("Could not load OTLP_HOST");

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(otlp_host),
        )
        .with_trace_config(
            opentelemetry::sdk::trace::config().with_resource(Resource::new(vec![KeyValue::new(
                "service.name",
                "foobot2",
            )])),
        )
        .install_batch(opentelemetry::runtime::Tokio)
        .unwrap();

    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let fmt_layer = tracing_subscriber::fmt::layer().compact();
    let filter_layer = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .expect("Failed to parse log directive");

    Registry::default()
        .with(fmt_layer)
        .with(telemetry)
        .with(filter_layer)
        .init()
}
