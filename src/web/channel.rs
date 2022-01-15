use crate::{database::models::NewCommand, platform::Permissions};

use super::api::ApiError;
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
    channel_id: u64,
) -> Html<Template> {
    let moderator = {
        if let Some(session) = &session {
            match cmd
                .get_permissions_in_channel(session.user_id, channel_id)
                .await
            {
                Ok(permissions) => {
                    tracing::info!("User permissions: {:?}", permissions);

                    permissions == Permissions::ChannelMod || permissions == Permissions::Admin
                }
                Err(_) => false,
            }
        } else {
            false
        }
    };
    tracing::debug!("Moderator: {}", moderator);

    Html(Template::render(
        "commands",
        &CommandsContext {
            parent_context: LayoutContext::new_with_auth(session),
            channel: channel_id,
            commands: cmd
                .db
                .get_commands(channel_id)
                .expect("Failed to get commands"),
            moderator,
        },
    ))
}

#[post("/<channel_id>/commands", data = "<command_form>")]
pub async fn update_command(
    command_form: Form<CommandForm>,
    cmd: &State<CommandHandler>,
    session: WebSession,
    channel_id: u64,
) -> Result<Redirect, ApiError> {
    tracing::info!("{:?}", command_form);

    let permissions = cmd
        .get_permissions_in_channel(session.user_id, channel_id)
        .await?;

    if permissions >= Permissions::ChannelMod {
        cmd.db.update_command(NewCommand {
            name: &command_form.trigger,
            action: &command_form.action,
            permissions: None,
            channel_id,
            cooldown: 5,
        })?;
    }

    Ok(Redirect::to(format!("/channels/{}/commands", channel_id)))
}

#[delete("/<channel_id>/commands", data = "<trigger>")]
pub async fn delete_command(
    channel_id: u64,
    session: WebSession,
    cmd: &State<CommandHandler>,
    trigger: &str,
) -> Result<(), ApiError> {
    let permissions = cmd
        .get_permissions_in_channel(session.user_id, channel_id)
        .await?;

    if permissions >= Permissions::ChannelMod {
        cmd.db.delete_command(channel_id, trigger)?;
    }
    Ok(())
}

#[catch(404)]
pub async fn not_found(_: &Request<'_>) -> Redirect {
    Redirect::to("/channels")
}

#[derive(FromForm, Debug)]
pub struct CommandForm {
    pub trigger: String,
    pub action: String,
}
