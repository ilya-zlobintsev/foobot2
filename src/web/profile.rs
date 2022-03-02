use rocket::get;
use rocket::response::status;
use rocket::response::{content::Html, Redirect};
use rocket::State;

use rocket_dyn_templates::Template;

use crate::database::models::User;
use crate::platform::ChannelIdentifier;
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

    let spotify_connected = cmd
        .db
        .get_spotify_access_token(session.user_id)
        .expect("DB Error")
        .is_some();

    let lastfm_name = cmd
        .db
        .get_lastfm_name(session.user_id)
        .expect("Missing user id");

    let mut admin = false;

    if let Ok(Some(admin_user)) = cmd.db.get_admin_user() {
        admin = admin_user.id == user.id;
    }

    let twitch_joined = match &user.twitch_id {
        Some(channel_id) => cmd
            .db
            .get_channel(&ChannelIdentifier::TwitchChannel((
                channel_id.clone(),
                None,
            )))
            .expect("DB error")
            .is_some(),
        None => false,
    };

    Ok(Html(Template::render(
        "profile",
        ProfileContext {
            admin,
            user,
            spotify_connected,
            lastfm_name,
            parent_context: LayoutContext::new_with_auth(Some(session)),
            twitch_joined,
        },
    )))
}

#[get("/join/twitch")]
pub async fn join_twitch(
    cmd: &State<CommandHandler>,
    user: User,
) -> Result<Redirect, status::BadRequest<&'static str>> {
    match user.twitch_id {
        Some(id) => {
            cmd.join_channel(&ChannelIdentifier::TwitchChannel((id, None)))
                .await
                .map_err(|e| {
                    tracing::warn!("Failed to join channel! {}", e);
                    status::BadRequest(Some("Failed to join channel"))
                })?;

            Ok(Redirect::to("/profile"))
        }
        None => Err(status::BadRequest(Some("Not logged in with Twitch!"))),
    }
}
