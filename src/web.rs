mod api;
pub mod authenticate;
mod channel;
mod errors;
mod profile;
mod template_context;
mod webhooks;
use anyhow::anyhow;
use dashmap::DashMap;
use tokio::task;

use std::env;

use reqwest::{Client, Response};
use rocket::{catchers, fs::FileServer, get, response::content::Html, routes, State};
use rocket_dyn_templates::Template;

use template_context::*;

use crate::{
    command_handler::{get_admin_channel, CommandHandler},
    database::models::WebSession,
};

#[get("/")]
async fn index(cmd: &State<CommandHandler>, session: Option<WebSession>) -> Html<Template> {
    let db = &cmd.db;

    tracing::debug!("{:?}", session);

    Html(Template::render(
        "index",
        &IndexContext {
            parent_context: LayoutContext::new_with_auth(session),
            channel_amount: db.get_channels_amount().expect("Failed to get channels"),
        },
    ))
}

pub async fn run(command_handler: CommandHandler) {
    let state_storage: DashMap<String, String> = DashMap::new();

    let rocket = rocket::build()
        .attach(Template::custom(|engines| {
            engines.handlebars.set_strict_mode(true);
        }))
        .mount("/static", FileServer::from("static"))
        .mount("/", routes![index])
        .mount(
            "/channels",
            routes![
                channel::index,
                channel::commands_page,
                channel::update_command,
                channel::delete_command
            ],
        )
        .mount(
            "/authenticate",
            routes![
                authenticate::index,
                authenticate::authenticate_twitch,
                authenticate::admin_authenticate_twitch_bot,
                authenticate::twitch_redirect,
                authenticate::admin_twitch_bot_redirect,
                authenticate::authenticate_discord,
                authenticate::discord_redirect,
                authenticate::authenticate_spotify,
                authenticate::spotify_redirect,
                authenticate::disconnect_spotify,
                authenticate::authenticate_twitch_manage,
                authenticate::twitch_manage_redirect,
                authenticate::logout,
            ],
        )
        .mount("/profile", routes![profile::profile, profile::join_twitch])
        .mount("/api", routes![api::set_lastfm_name])
        .mount("/hooks", routes![webhooks::eventsub_callback])
        .register("/", catchers![errors::not_found, errors::not_authorized])
        .register("/channels", catchers![channel::not_found])
        .manage(Client::new())
        .manage(command_handler.clone())
        .manage(state_storage)
        .ignite()
        .await
        .expect("Failed to ignite rocket");

    let shutdown_handle = rocket.shutdown();

    task::spawn(async move {
        shutdown_handle.await;

        if let Some(admin_channel) = get_admin_channel() {
            command_handler
                .send_to_channel(
                    admin_channel,
                    format!("Foobot2 {} Shutting down...", crate::get_version()),
                )
                .await
                .expect("Failed to send shutdown message");
        }
    });

    rocket.launch().await.expect("Failed to launch web server")
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
