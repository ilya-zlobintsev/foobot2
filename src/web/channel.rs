use std::collections::HashMap;

use crate::{
    database::models::NewCommand,
    platform::{ChannelIdentifier, Permissions},
};

use super::api::ApiError;
use super::*;

use futures::future::join_all;
use rocket::{catch, form::Form, get, post, response::Redirect, Request, State};
use rocket_dyn_templates::Template;
use std::sync::Arc;
use tokio::sync::Mutex;

#[get("/")]
pub async fn index(
    cmd: &State<CommandHandler>,
    session: Option<WebSession>,
) -> Result<Html<Template>, ApiError> {
    let base_channels = cmd.db.get_channels().expect("DB error");
    let mut friendly_names =
        get_friendly_names(base_channels.iter().map(|ch| ch.id).collect(), &*cmd).await?;

    Ok(Html(Template::render(
        "channels",
        &ChannelsContext {
            parent_context: LayoutContext::new_with_auth(session),
            channels: base_channels
                .into_iter()
                .map(|channel| PublicChannel {
                    name: friendly_names.remove(&channel.id),
                    channel,
                })
                .collect(),
        },
    )))
}

#[get("/<channel_id>/commands")]
pub async fn commands_page(
    cmd: &State<CommandHandler>,
    session: Option<WebSession>,
    channel_id: u64,
) -> Result<Html<Template>, ApiError> {
    let channel = cmd
        .db
        .get_channel_by_id(channel_id)?
        .ok_or(ApiError::NotFound)?;

    let channel_identifier = ChannelIdentifier::new(&channel.platform, channel.channel).unwrap();

    let moderator = {
        if let Some(session) = &session {
            let user = cmd
                .db
                .get_user_by_id(session.user_id)?
                .expect("Invalid user");

            match cmd
                .get_permissions_in_channel(user, &channel_identifier)
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

    Ok(Html(Template::render(
        "commands",
        &CommandsContext {
            parent_context: LayoutContext::new_with_auth(session),
            channel: channel_id,
            commands: cmd
                .db
                .get_commands(channel_id)
                .expect("Failed to get commands"),
            moderator,
            eventsub_triggers: match channel_identifier {
                ChannelIdentifier::TwitchChannel((broadcaster_id, _)) => cmd
                    .db
                    .get_eventsub_triggers_for_broadcaster(&broadcaster_id)?,
                _ => Vec::new(),
            },
        },
    )))
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

async fn get_friendly_names(
    channel_ids: Vec<u64>,
    cmd: &CommandHandler,
) -> Result<HashMap<u64, String>, ApiError> {
    // Assigns the platform's ids to channel ids
    let mut twitch_channels = HashMap::new();
    let mut discord_channels = HashMap::new();

    for id in channel_ids {
        match cmd.db.get_channel_by_id(id)? {
            Some(channel) => match channel.get_identifier() {
                ChannelIdentifier::TwitchChannel((twitch_id, _)) => {
                    twitch_channels.insert(twitch_id, id);
                }
                ChannelIdentifier::DiscordChannel(guild_id) => {
                    discord_channels.insert(guild_id, id);
                }
                _ => (),
            },
            None => return Err(ApiError::NotFound),
        }
    }

    tracing::trace!("twitch channels: {:?}", twitch_channels);

    let results = Arc::new(Mutex::new(HashMap::new()));

    let platform_handler = cmd.platform_handler.read().await;
    let helix = platform_handler
        .twitch_api
        .as_ref()
        .expect("Twitch API not initialized even though there are Twitch channels registered")
        .helix_api
        .clone();
    let discord_api = platform_handler
        .discord_api
        .clone()
        .expect("Discord API not initialized even though there are Discord channels registered");

    let mut handles = Vec::new();

    {
        let results = results.clone();
        handles.push(tokio::spawn(async move {
            match helix
                .get_users(
                    None,
                    Some(&twitch_channels.keys().map(|s| s.as_str()).collect()),
                )
                .await
            {
                Ok(users) => {
                    let mut results = results.lock().await;

                    users.into_iter().for_each(|twitch_user| {
                        tracing::trace!("{:?}", twitch_user);
                        let channel_id = *twitch_channels.get(&twitch_user.id).unwrap();

                        results.insert(channel_id, twitch_user.display_name);
                    });
                }
                Err(e) => tracing::error!("Error getting Twitch name: {}", e),
            }
        }));
    }

    for (guild_id, id) in discord_channels {
        let discord_api = discord_api.clone();
        let results = results.clone();
        handles.push(tokio::spawn(async move {
            match discord_api.get_guild(guild_id.parse().unwrap()).await {
                Ok(guild) => {
                    results.lock().await.insert(id, guild.name);
                }
                Err(e) => tracing::error!("Error getting guild: {}", e),
            }
        }));
    }
    join_all(handles).await;

    let results = results.lock().await.clone();
    Ok(results)
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
