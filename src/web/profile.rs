use rocket::get;
use rocket::response::{content::Html, Redirect};
use rocket::State;

use rocket_dyn_templates::Template;

use crate::{command_handler::CommandHandler, database::models::WebSession};

use super::template_context::{LayoutContext, ProfileContext};

#[get("/")]
pub fn profile(
    cmd: &State<CommandHandler>,
    session: WebSession,
) -> Result<Html<Template>, Redirect> {
    let user = cmd
        .db
        .get_user_by_id(session.user_id)
        .expect("DB Error")
        .expect("Potentially invalid user session");

    let spotify_connected = match cmd
        .db
        .get_spotify_access_token(session.user_id)
        .expect("DB Error")
    {
        Some(_) => true,
        None => false,
    };

    let lastfm_name = cmd
        .db
        .get_lastfm_name(session.user_id)
        .expect("Missing user id");

    Ok(Html(Template::render(
        "profile",
        ProfileContext {
            user,
            spotify_connected,
            lastfm_name,
            parent_context: LayoutContext::new_with_auth(Some(session)),
        },
    )))
}
