use crate::platform::Permissions;

use super::api::{get_permissions, ApiError};
use super::*;

use rocket::{catch, form::Form, get, post, response::Redirect, Request, State};
use rocket_dyn_templates::Template;

#[get("/")]
pub async fn index(cmd: &State<CommandHandler>, session: Option<WebSession>) -> Html<Template> {
    Html(Template::render(
        "channels",
        &ChannelsContext {
            parent_context: LayoutContext::new_with_auth(session),
            channels: cmd.db.get_channels().expect("Failed to get channels"),
        },
    ))
}

#[get("/<channel_id>/commands")]
pub async fn commands_page(
    cmd: &State<CommandHandler>,
    session: Option<WebSession>,
    channel_id: String,
) -> Html<Template> {
    Html(Template::render(
        "commands",
        &CommandsContext {
            parent_context: LayoutContext::new_with_auth(session),
            channel: channel_id.clone(),
            commands: cmd
                .db
                .get_commands(channel_id.parse().unwrap())
                .expect("Failed to get commands"),
        },
    ))
}

#[post("/<channel_id>/commands", data = "<command_form>")]
pub async fn add_command(
    command_form: Form<AddCommandForm>,
    cmd: &State<CommandHandler>,
    session: WebSession,
    channel_id: String,
) -> Result<Redirect, ApiError> {
    let permissions = get_permissions(&channel_id, session, cmd).await?;
    

    if permissions == Permissions::ChannelMod {
        cmd.db.add_command_to_channel_id(
            channel_id.parse().unwrap(),
            &command_form.cmd_trigger,
            &command_form.cmd_action,
        )?;
    }

    Ok(Redirect::to(format!("/channels/{}/commands", channel_id)))
}

#[catch(404)]
pub async fn not_found(_: &Request<'_>) -> Redirect {
    Redirect::to("/channels")
}

#[derive(FromForm)]
pub struct AddCommandForm {
    pub cmd_trigger: String,
    pub cmd_action: String,
}
