mod authenticate;
mod channel;
mod errors;
mod template_context;

use reqwest::Client;
use rocket::{catchers, get, http::CookieJar, routes, State};
use rocket_contrib::templates::Template;
use tokio::task::{self, JoinHandle};

use template_context::*;

use crate::{command_handler::CommandHandler, database::Database};

#[get("/")]
async fn index(db: &State<Database>, jar: &CookieJar<'_>) -> Template {
    Template::render(
        "index",
        &IndexContext {
            parent_context: LayoutContext::new(db, jar),
            channel_amount: db.get_channels_amount().expect("Failed to get channels"),
        },
    )
}

pub async fn run(command_handler: CommandHandler) -> JoinHandle<()> {
    let mut rocket = rocket::build()
        .attach(Template::custom(|engines| {
            engines.handlebars.set_strict_mode(true);
        }))
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
            ],
        )
        .register("/", catchers![errors::not_found])
        .register("/channels", catchers![channel::not_found])
        .manage(Client::new())
        .manage(command_handler.db);

    if let Some(twitch_api) = command_handler.twitch_api {
        rocket = rocket.manage(twitch_api);
    }

    task::spawn(async { rocket.launch().await.expect("Failed to launch web server") })
}
