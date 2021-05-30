mod api;
mod authenticate;
mod channel;
mod errors;
mod profile;
mod template_context;

use reqwest::Client;
use rocket::{State, catchers, fs::FileServer, get, http::CookieJar, response::content::Html, routes};
use rocket_dyn_templates::Template;
use tokio::task::{self, JoinHandle};

use template_context::*;

use crate::command_handler::CommandHandler;

#[get("/")]
async fn index(cmd: &State<CommandHandler>, jar: &CookieJar<'_>) -> Html<Template> {
    let db = &cmd.db;

    Html(Template::render(
        "index",
        &IndexContext {
            parent_context: LayoutContext::new(db, jar),
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
            .mount("/channels", routes![channel::index, channel::commands_page])
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
            .register("/", catchers![errors::not_found])
            .register("/channels", catchers![channel::not_found])
            .manage(Client::new())
            .manage(command_handler)
            .launch()
            .await
            .expect("Failed to launch web server")
    })
}
