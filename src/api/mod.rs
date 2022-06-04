mod authentication;
mod channels;
mod error;
mod webhooks;

use anyhow::anyhow;
use dashmap::DashMap;
use reqwest::{Client, Response};
use rocket::{
    fs::{FileServer, NamedFile},
    response::status,
};
use rocket_okapi::{
    mount_endpoints_and_merged_docs, openapi_get_routes_spec, rapidoc, settings::UrlObject,
};
use rocket_prometheus::PrometheusMetrics;
use std::{env, path::PathBuf};
use tokio::task;

use self::error::ApiError;
use crate::command_handler::{get_admin_channel, CommandHandler};

type Result<T> = std::result::Result<T, ApiError>;

pub async fn run(command_handler: CommandHandler) {
    let prometheus = PrometheusMetrics::new();

    prometheus
        .registry()
        .register(Box::new(crate::command_handler::COMMAND_COUNTER.clone()))
        .unwrap();

    prometheus
        .registry()
        .register(Box::new(
            crate::command_handler::COMMAND_PROCESSING_HISTOGRAM.clone(),
        ))
        .unwrap();

    let state_storage: DashMap<String, String> = DashMap::new();

    let mut building_rocket = rocket::build()
        .attach(prometheus.clone())
        .manage(Client::new())
        .manage(command_handler.clone())
        .manage(state_storage)
        .mount("/", FileServer::from("web/public"))
        .mount("/", routes![get_index])
        .mount(
            "/authenticate",
            routes![
                authentication::flow::authenticate_twitch,
                authentication::flow::admin_authenticate_twitch_bot,
                authentication::flow::twitch_redirect,
                authentication::flow::admin_twitch_bot_redirect,
                authentication::flow::authenticate_discord,
                authentication::flow::discord_redirect,
                authentication::flow::authenticate_spotify,
                authentication::flow::spotify_redirect,
                authentication::flow::authenticate_twitch_manage,
                authentication::flow::twitch_manage_redirect,
            ],
        )
        .mount(
            "/api/doc/",
            rapidoc::make_rapidoc(&rapidoc::RapiDocConfig {
                title: Some("Foobot | RapiDoc".to_owned()),
                general: rapidoc::GeneralConfig {
                    spec_urls: vec![UrlObject::new("General", "../openapi.json")],
                    ..Default::default()
                },
                hide_show: rapidoc::HideShowConfig {
                    allow_spec_url_load: false,
                    allow_spec_file_load: false,
                    ..Default::default()
                },
                ui: rapidoc::UiConfig {
                    theme: rapidoc::Theme::Dark,
                    ..Default::default()
                },
                ..Default::default()
            }),
        );

    let openapi_settings = rocket_okapi::settings::OpenApiSettings::default();
    mount_endpoints_and_merged_docs! {
        building_rocket, "/api".to_owned(), openapi_settings,
        "/session" => openapi_get_routes_spec![
            authentication::api::get_session,
            authentication::api::get_user,
            authentication::api::logout,
            authentication::api::set_lastfm_name,
            authentication::api::disconnect_spotify,
        ],
        "/channels" => openapi_get_routes_spec![
            channels::get_channels,
            channels::get_channel_count,
            channels::get_channel_info,
            channels::get_channel_commands,
            channels::get_channel_eventsub_triggers,
            channels::get_filters,
        ],
        "/hooks" => openapi_get_routes_spec![webhooks::eventsub_callback],
    };

    let rocket = building_rocket
        .ignite()
        .await
        .expect("Failed to ignite rocket");

    let shutdown_handle = rocket.shutdown();

    task::spawn(async move {
        shutdown_handle.await;

        if let Some(admin_channel) = get_admin_channel() {
            command_handler
                .platform_handler
                .read()
                .await
                .send_to_channel(
                    admin_channel,
                    format!("Foobot2 {} Shutting down...", crate::get_version()),
                )
                .await
                .expect("Failed to send shutdown message");
        }
    });

    let _ = rocket.launch().await.expect("Failed to launch web server");
}

pub fn get_base_url() -> String {
    env::var("BASE_URL").expect("BASE_URL missing!")
}

pub fn response_ok(r: &Response) -> anyhow::Result<()> {
    if r.status().is_success() {
        Ok(())
    } else {
        Err(anyhow!("Non-success response: {}", r.status()))
    }
}

#[get("/<path..>", rank = 15)]
async fn get_index(
    path: PathBuf,
) -> std::result::Result<NamedFile, status::NotFound<&'static str>> {
    if path.to_str().map_or(false, |s| s.starts_with("api")) {
        Err(status::NotFound("API endpoint not found"))
    } else {
        Ok(NamedFile::open("web/public/index.html").await.unwrap())
    }
}
