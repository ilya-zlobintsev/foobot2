use crate::{
    command_handler::twitch_api::eventsub::{conditions::*, EventSubSubscriptionType},
    database::models::NewCommand,
    platform::Permissions,
};

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
                .get_permissions_in_channel_by_id(session.user_id, channel_id)
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
        .get_permissions_in_channel_by_id(session.user_id, channel_id)
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
        .get_permissions_in_channel_by_id(session.user_id, channel_id)
        .await?;

    if permissions >= Permissions::ChannelMod {
        cmd.db.delete_command(channel_id, trigger)?;
    }
    Ok(())
}

// #[post("/<channel_id>/eventsub", data = "<trigger_form>")]
// pub async fn add_eventsub_trigger(
//     cmd: &State<CommandHandler>,
//     channel_id: u64,
//     trigger_form: Form<EventSubTriggerForm>,
// ) -> Result<(), ApiError> {
//     let channel = cmd
//         .db
//         .get_channel_by_id(channel_id)?
//         .ok_or_else(|| ApiError::GenericError("Channel not found".to_string()))?;

//     if channel.platform == "twitch" {
//         let broadcaster_id = channel.channel;

//         let sub = match trigger_form.event.as_str() {
//             "channel.update" => EventSubSubscriptionType::ChannelUpdate(ChannelUpdateCondition {
//                 broadcaster_user_id: broadcaster_id,
//             }),
//             _ => {
//                 return Err(ApiError::GenericError(
//                     "Unrecognized event type".to_string(),
//                 ))
//             }
//         };

//         let twitch_api = cmd.twitch_api.as_ref().expect("Twitch not initialized"); // Expect because this only triggers if you have a twitch channel but no twitch_api
        
//         twitch_api.add_eventsub_trigger().await?

//         Ok(())
//     } else {
//         Err(ApiError::GenericError(
//             "Channel is not on Twitch".to_string(),
//         ))
//     }
// }

#[catch(404)]
pub async fn not_found(_: &Request<'_>) -> Redirect {
    Redirect::to("/channels")
}

#[derive(FromForm, Debug)]
pub struct CommandForm {
    pub trigger: String,
    pub action: String,
}

#[derive(FromForm, Debug)]
pub struct EventSubTriggerForm {
    pub event: String,
    pub action: String,
}
