use crate::platform::{ChannelIdentifier, Permissions};

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
    channel_id: String,
) -> Html<Template> {
    let moderator = {
        if let Some(session) = &session {
            match get_permissions(&channel_id, session.user_id, cmd).await {
                Ok(permissions) => {
                    tracing::info!("User permissions: {:?}", permissions);

                    permissions == Permissions::ChannelMod || permissions == Permissions::Admin
                }
                None => false,
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
            channel: channel_id.clone(),
            commands: cmd
                .db
                .get_commands(channel_id.parse().unwrap())
                .expect("Failed to get commands"),
            moderator,
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
    let permissions = get_permissions(&channel_id, session.user_id, cmd).await?;

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

pub async fn get_permissions(
    channel_id: &str,
    user_id: u64,
    cmd: &CommandHandler,
) -> Result<Permissions, ApiError> {
    let user = cmd
        .db
        .get_user_by_id(user_id)?
        .ok_or_else(|| ApiError::InvalidUser)?;

    if let Ok(Some(admin_user)) = cmd.db.get_admin_user() {
        if user.id == admin_user.id {
            return Ok(Permissions::Admin);
        }
    }

    match cmd
        .db
        .get_channel_by_id(channel_id.parse().expect("Invalid Channel ID"))?
    {
        Some(channel) => match ChannelIdentifier::new(&channel.platform, channel.channel)? {
            ChannelIdentifier::TwitchChannelID(channel_id) => {
                let twitch_id = user.twitch_id.ok_or_else(|| {
                    ApiError::GenericError("Not registered on this platform".to_string())
                })?;

                let twitch_api = cmd
                    .twitch_api
                    .as_ref()
                    .ok_or_else(|| ApiError::GenericError("Twitch not configured".to_string()))?;

                let users_response = twitch_api.get_users(None, Some(&vec![&channel_id])).await?;

                let channel_login = &users_response.first().expect("User not found").login;

                match twitch_api.get_channel_mods(&channel_login).await?.contains(
                    &twitch_api
                        .get_users(None, Some(&vec![&twitch_id]))
                        .await?
                        .first()
                        .unwrap()
                        .display_name,
                ) {
                    true => Ok(Permissions::ChannelMod),
                    false => Ok(Permissions::Default),
                }
            }
            ChannelIdentifier::DiscordGuildID(guild_id) => {
                let user_id = user
                    .discord_id
                    .ok_or_else(|| ApiError::InvalidUser)?
                    .parse()
                    .unwrap();

                let discord_api = cmd.discord_api.as_ref().unwrap();

                match discord_api
                    .get_permissions_in_guild(user_id, guild_id.parse().unwrap())
                    .await
                    .map_err(|_| ApiError::GenericError("discord error".to_string()))?
                    .contains(twilight_model::guild::Permissions::ADMINISTRATOR)
                {
                    true => Ok(Permissions::ChannelMod),
                    false => Ok(Permissions::Default),
                }
            }
            ChannelIdentifier::Anonymous => Ok(Permissions::Default),
        },
        None => Ok(Permissions::Default),
    }
}
