use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::state::AppState;
use super::Result;
use crate::api::error::ApiError;
use crate::command_handler::{CommandHandler, ExecutionContext};
use crate::database;
use crate::database::models::{Command, CommandMode, Filter, User, WebSession};
use crate::platform::{ChannelIdentifier, Permissions, ServerPlatformContext, UserIdentifier};

pub async fn get_channels(cmd: State<CommandHandler>) -> Result<Json<Vec<Channel>>> {
    let base_channels = cmd.db.get_channels().expect("DB error");
    let mut friendly_names =
        get_friendly_names(base_channels.iter().map(|ch| ch.id).collect(), &cmd).await?;

    let channels = base_channels
        .into_iter()
        .map(|channel| Channel {
            display_name: friendly_names.remove(&channel.id),
            channel,
        })
        .collect();

    Ok(Json(channels))
}

pub async fn get_channel_info(
    Path(channel_id): Path<u64>,
    cmd: State<CommandHandler>,
    user: Option<User>,
) -> Result<Json<ChannelInfo>> {
    let channel = cmd
        .db
        .get_channel_by_id(channel_id)?
        .ok_or(ApiError::NotFound)?;

    let display_name = match get_channel_display_name(&channel, &cmd).await {
        Ok(name) => name,
        Err(error) => {
            tracing::error!("Failed to query the display name: {error:?}");
            None
        }
    };

    let permissions = match user {
        Some(user) => {
            let permissions = cmd
                .get_permissions_in_channel(user, &channel.get_identifier())
                .await
                .unwrap_or_else(|error| {
                    tracing::error!("Failed to query permissions: {error}");
                    Permissions::Default
                });
            Some(PermissionsInfo {
                name: permissions,
                value: permissions as usize,
            })
        }
        None => None,
    };

    // (Route, link name)
    let extra_sections = match channel.get_identifier() {
        ChannelIdentifier::TwitchChannel(_) => vec![("./eventsub", "Eventsub")],
        _ => vec![],
    };

    Ok(Json(ChannelInfo {
        id: channel_id,
        display_name,
        permissions,
        extra_sections,
    }))
}

pub async fn get_channel_commands(
    Path(channel_id): Path<u64>,
    cmd: State<CommandHandler>,
) -> Result<Json<Vec<Command>>> {
    Ok(Json(cmd.db.get_commands(channel_id)?))
}

pub async fn get_channel_eventsub_triggers(
    Path(channel_id): Path<u64>,
    cmd: State<CommandHandler>,
) -> Result<Json<Vec<Value>>> {
    let channel = cmd
        .db
        .get_channel_by_id(channel_id)?
        .ok_or(ApiError::NotFound)?;

    match channel.get_identifier() {
        ChannelIdentifier::TwitchChannel((id, _)) => {
            let platform_handler = cmd.platform_handler.read().await;
            let helix = platform_handler
                .twitch_api
                .as_ref()
                .map(|twitch_api| &twitch_api.helix_api)
                .expect("Twitch not configured");

            let twitch_user = helix.get_user_by_id(&id).await?;

            let triggers = cmd
                .db
                .get_eventsub_triggers_for_broadcaster(&twitch_user.id)?
                .into_iter()
                .map(|trigger| {
                    json!({
                       "event_type": trigger.event_type,
                       "condition": trigger.creation_payload,
                       "action": trigger.action,
                       "mode": trigger.mode,
                    })
                })
                .collect();

            Ok(Json(triggers))
        }
        _ => Err(ApiError::BadRequest("Not a Twitch channel".to_owned())),
    }
}

pub async fn get_filters(
    session: WebSession,
    Path(channel_id): Path<u64>,
    cmd: State<CommandHandler>,
) -> Result<Json<Vec<Filter>>> {
    if cmd
        .get_permissions_in_channel_by_id(session.user_id, channel_id)
        .await?
        >= Permissions::ChannelMod
    {
        Ok(Json(cmd.db.get_filters_in_channel_id(channel_id)?))
    } else {
        Err(ApiError::Unauthorized(
            "Not a moderator in this channel".to_owned(),
        ))
    }
}

pub async fn get_channel_count(cmd: State<CommandHandler>) -> Result<Json<i64>> {
    Ok(Json(cmd.db.get_channels_amount()?))
}

#[derive(Serialize)]
pub struct Channel {
    #[serde(flatten)]
    pub channel: crate::database::models::Channel,
    pub display_name: Option<String>,
}

async fn get_friendly_names(
    channel_ids: Vec<u64>,
    cmd: &CommandHandler,
) -> Result<HashMap<u64, String>> {
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
    let mut handles = Vec::new();

    if !twitch_channels.is_empty() {
        let helix = platform_handler
            .twitch_api
            .as_ref()
            .expect("Twitch API not initialized even though there are Twitch channels registered")
            .helix_api
            .clone();

        let results = results.clone();
        handles.push(tokio::spawn(async move {
            let channels: Vec<_> = twitch_channels.keys().map(|s| s.as_str()).collect();
            match helix.get_users(None, Some(&channels)).await {
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

    if !discord_channels.is_empty() {
        let discord_api = platform_handler.discord_api.clone().expect(
            "Discord API not initialized even though there are Discord channels registered",
        );

        for (guild_id, id) in discord_channels {
            let discord_api = discord_api.clone();
            let results = results.clone();
            handles.push(tokio::spawn(async move {
                match discord_api.get_guild_name(guild_id.parse().unwrap()).await {
                    Ok(name) => {
                        results.lock().await.insert(id, name);
                    }
                    Err(e) => tracing::error!("Error getting guild: {}", e),
                }
            }));
        }
    }
    join_all(handles).await;

    let results = results.lock().await.clone();
    Ok(results)
}

#[derive(Serialize)]
pub struct ChannelInfo {
    pub id: u64,
    pub display_name: Option<String>,
    pub permissions: Option<PermissionsInfo>,
    pub extra_sections: Vec<(&'static str, &'static str)>,
}

#[derive(Serialize)]
pub struct PermissionsInfo {
    pub name: Permissions,
    pub value: usize,
}

async fn get_channel_display_name(
    channel: &database::models::Channel,
    cmd: &CommandHandler,
) -> Result<Option<String>> {
    let platform_handler_guard = cmd.platform_handler.read().await;

    match channel.platform.as_str() {
        "twitch" => {
            let twitch_api = platform_handler_guard
                .twitch_api
                .as_ref()
                .expect("Twitch channel found but Twitch is not configured");

            let twitch_user = twitch_api
                .helix_api
                .get_user_by_id(&channel.channel)
                .await?;

            Ok(Some(twitch_user.display_name))
        }
        "discord_guild" => {
            let discord_api = platform_handler_guard
                .discord_api
                .as_ref()
                .expect("Discord channel found but DIscord is not configured");

            let guild_name = discord_api
                .get_guild_name(channel.channel.parse().unwrap())
                .await?;

            Ok(Some(guild_name))
        }
        _ => Ok(None),
    }
}

#[derive(Deserialize)]
pub struct EvalParams {
    pub mode: String,
    pub args: Vec<String>,
}

pub async fn eval(
    Path(channel_id): Path<u64>,
    user: User,
    Query(EvalParams { mode, args }): Query<EvalParams>,
    cmd: State<CommandHandler>,
    payload: String,
) -> Result<String> {
    let channel = cmd
        .db
        .get_channel_by_id(channel_id)?
        .ok_or(ApiError::NotFound)?;

    if cmd
        .get_permissions_in_channel(user.clone(), &channel.get_identifier())
        .await?
        >= Permissions::ChannelMod
    {
        let command_mode = CommandMode::from_str(&mode)
            .map_err(|_| ApiError::BadRequest(format!("Invalid command mode {mode}")))?;

        let executing_user = if let Some(twitch_id) = user.twitch_id.clone() {
            UserIdentifier::TwitchID(twitch_id)
        } else if let Some(local_ip) = &user.local_addr {
            UserIdentifier::IpAddr(local_ip.parse().unwrap())
        } else {
            todo!()
        };

        let platform_ctx = ServerPlatformContext {
            target_channel: channel.get_identifier(),
            executing_user,
            cmd: cmd.0.clone(),
            display_name: "Tester via API".to_owned(),
        };

        let processing_timestamp = Utc::now();
        let platform_handler = cmd.platform_handler.read().await;
        let execution_ctx = ExecutionContext {
            db: &cmd.db,
            platform_handler: &platform_handler,
            platform_ctx,
            user: &user,
            processing_timestamp,
            blocked_users: &cmd.blocked_users,
        };

        let command = Command {
            name: "EVAL testing".to_owned(),
            action: payload,
            permissions: None,
            channel_id: channel.id,
            triggers: None,
            cooldown: Some(0),
            mode: command_mode,
        };
        let response = cmd
            .execute_command(command, &execution_ctx, args)
            .await?
            .unwrap_or_else(|| "<empty response>".to_owned());
        Ok(response)
    } else {
        Err(ApiError::Unauthorized(
            "Not a moderator in this channel".to_owned(),
        ))
    }
}

pub fn create_router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_channels))
        .route("/count", get(get_channel_count))
        .route("/:id/info", get(get_channel_info))
        .route("/:id/filters", get(get_filters))
        .route("/:id/eventsub", get(get_channel_eventsub_triggers))
        .route("/:id/commands", get(get_channel_commands))
        .route("/:id/eval", post(eval))
}
