mod api;
mod authenticate;
mod channel;
mod errors;
mod profile;
mod webhooks;
mod template_context;

use reqwest::Client;
use rocket::{
    catchers, fs::FileServer, get, response::content::Html, routes, State,
};
use rocket_dyn_templates::Template;
use tokio::task::{self, JoinHandle};

use template_context::*;

use crate::{command_handler::CommandHandler, database::models::WebSession};

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

pub async fn run(command_handler: CommandHandler) -> JoinHandle<()> {
    task::spawn(async {
        rocket::build()
            .attach(Template::custom(|engines| {
                engines.handlebars.set_strict_mode(true);
            }))
            .mount("/static", FileServer::from("static"))
            .mount("/", routes![index])
            .mount(
                "/channels",
                routes![channel::index, channel::commands_page, channel::add_command],
            )
            .mount(
                "/authenticate",
                routes![
                    authenticate::index,
                    authenticate::authenticate_twitch,
                    authenticate::twitch_redirect,
                    authenticate::authenticate_discord,
                    authenticate::discord_redirect,
                    authenticate::authenticate_spotify,
                    authenticate::spotify_redirect,
                    authenticate::disconnect_spotify,
                    authenticate::logout,
                ],
            )
            .mount("/profile", routes![profile::profile])
            .mount("/api", routes![api::get_permissions])
            .mount("/webhooks", routes![webhooks::twitch_callback])
            .register("/", catchers![errors::not_found, errors::not_authorized])
            .register("/channels", catchers![channel::not_found])
            .manage(Client::new())
            .manage(command_handler)
            .launch()
            .await
            .expect("Failed to launch web server")
    })
}
