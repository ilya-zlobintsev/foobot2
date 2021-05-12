use super::*;
use rocket::{catch, get, response::Redirect, Request, State};
use rocket_contrib::templates::Template;

#[get("/")]
pub async fn index(db: &State< Database>) -> Template {
    Template::render(
        "channels",
        &ChannelsContext {
            parent: "layout",
            channels: db.get_channels().expect("Failed to get channels"),
        },
    )
}

#[get("/<channel_id>/commands")]
pub async fn commands_page(db: &State< Database>, channel_id: String) -> Template {
    Template::render(
        "commands",
        &CommandsContext {
            parent: "layout",
            channel: channel_id.clone(),
            commands: db
                .get_commands(channel_id.parse().unwrap())
                .expect("Failed to get commands"),
        },
    )
}

#[catch(404)]
pub async fn not_found(_: &Request<'_>) -> Redirect {
    Redirect::to("/channels")
}
