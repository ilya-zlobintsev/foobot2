use rocket::{
    get,
    http::CookieJar,
    response::{content::Html, Redirect},
    State,
};
use rocket_dyn_templates::Template;

use crate::command_handler::CommandHandler;

use super::template_context::{AuthInfo, LayoutContext, ProfileContext};

#[get("/")]
pub fn profile(
    cmd: &State<CommandHandler>,
    jar: &CookieJar<'_>,
) -> Result<Html<Template>, Redirect> {
    match AuthInfo::new(&cmd.db, jar) {
        Some(auth_info) => {
            let user = cmd
                .db
                .get_user_by_id(auth_info.user_id)
                .expect("DB Error")
                .expect("Potentially invalid user session");

            let spotify_connected = match cmd
                .db
                .get_spotify_access_token(auth_info.user_id)
                .expect("DB Error")
            {
                Some(_) => true,
                None => false,
            };

            Ok(Html(Template::render(
                "profile",
                ProfileContext {
                    user,
                    spotify_connected,
                    parent_context: LayoutContext::new_with_auth(Some(auth_info)),
                },
            )))
        }
        None => Err(Redirect::to("/authenticate")),
    }
}
