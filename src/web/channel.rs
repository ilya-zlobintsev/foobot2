use super::*;
use rocket::{catch, get, http::CookieJar, response::Redirect, Request, State};
use rocket_dyn_templates::Template;

#[get("/")]
pub async fn index(cmd: &State<CommandHandler>, jar: &CookieJar<'_>) -> Html<Template> {
    Html(Template::render(
        "channels",
        &ChannelsContext {
            parent_context: LayoutContext::new(&cmd.db, jar),
            channels: cmd.db.get_channels().expect("Failed to get channels"),
        },
    ))
}

#[get("/<channel_id>/commands")]
pub async fn commands_page(
    cmd: &State<CommandHandler>,
    jar: &CookieJar<'_>,
    channel_id: String,
) -> Html<Template> {
    Html(Template::render(
        "commands",
        &CommandsContext {
            parent_context: LayoutContext::new(&cmd.db, jar),
            channel: channel_id.clone(),
            commands: cmd
                .db
                .get_commands(channel_id.parse().unwrap())
                .expect("Failed to get commands"),
        },
    ))
}

#[catch(404)]
pub async fn not_found(_: &Request<'_>) -> Redirect {
    Redirect::to("/channels")
}
