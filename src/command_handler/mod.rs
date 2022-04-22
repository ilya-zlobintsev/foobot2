mod commands;
pub mod discord_api;
pub mod finnhub_api;
pub mod inquiry_helper;
pub mod lastfm_api;
pub mod lingva_api;
pub mod owm_api;
pub mod platform_handler;
pub mod spotify_api;
pub mod twitch_api;

use anyhow::{anyhow, Context};
use core::fmt;
use dashmap::DashMap;
use discord_api::DiscordApi;
use handlebars::Handlebars;
use inquiry_helper::*;
use lastfm_api::LastFMApi;
use lingva_api::LingvaApi;
use owm_api::OwmApi;
use reqwest::Client;
use std::collections::HashMap;
use std::env::{self, VarError};
use std::num::ParseIntError;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::vec::IntoIter;
use tokio::sync::{Mutex, RwLock};
use tokio::task;
use twitch_api::TwitchApi;
use twitch_irc::login::{LoginCredentials, RefreshingLoginCredentials};

use self::commands::Command;
use self::finnhub_api::FinnhubApi;
use self::platform_handler::PlatformHandler;
use self::twitch_api::eventsub::conditions::*;
use self::twitch_api::eventsub::EventSubSubscriptionType;
use self::twitch_api::helix::HelixApi;
use self::twitch_api::{get_client_id, get_client_secret};
use crate::api;
use crate::command_handler::commands::builtin::BuiltinCommand;
use crate::database::models::{Filter, NewEventSubTrigger};
use crate::database::DatabaseError;
use crate::database::{models::User, Database};
use crate::platform::minecraft;
use crate::platform::{
    twitch, ChannelIdentifier, ExecutionContext, Permissions, ServerExecutionContext,
    UserIdentifierError,
};

#[derive(Clone)]
pub struct CommandHandler {
    pub db: Database,
    pub platform_handler: Arc<RwLock<PlatformHandler>>,
    startup_time: Arc<Instant>,
    template_registry: Arc<Handlebars<'static>>,
    cooldowns: Arc<RwLock<Vec<(u64, String)>>>, // User id and command
    command_triggers: Arc<DashMap<u64, Arc<DashMap<String, String>>>>, // Channel id, trigger phrase and command name
    mirror_connections: Arc<HashMap<String, ChannelIdentifier>>,       // from and to channel
}

impl CommandHandler {
    pub async fn init(db: Database) -> Self {
        let twitch_api = match TwitchApi::init_refreshing(db.clone()).await {
            Ok(api) => {
                let active_triggers = api
                    .helix_api_app
                    .get_eventsub_subscriptions(None)
                    .await
                    .expect("Failed to get EventSub triggers");

                for trigger in db.get_eventsub_triggers().expect("DB Error") {
                    if !active_triggers
                        .iter()
                        .any(|active_trigger| active_trigger.id == trigger.id)
                    {
                        let subscription_type = serde_json::from_str(&trigger.creation_payload)
                            .expect("Deserialization error");

                        match api
                            .helix_api_app
                            .add_eventsub_subscription(subscription_type)
                            .await
                        {
                            Ok(response) => {
                                let new_id = &response.data.first().unwrap().id;

                                db.update_eventsub_trigger_id(&trigger.id, new_id)
                                    .expect("DB error");
                            }
                            Err(e) => tracing::error!("Failed to add EventSub subscription! {}", e),
                        }
                    }
                }

                Some(api)
            }
            Err(e) => {
                tracing::info!("Failed to initialize Twitch API: {}", e);
                None
            }
        };

        let discord_api = match env::var("DISCORD_TOKEN") {
            Ok(token) => Some(DiscordApi::new(token)),
            Err(_) => None,
        };

        let lingva_url = match env::var("LINGVA_INSTANCE_URL") {
            Ok(url) => url,
            Err(_) => "https://lingva.ml".to_owned(),
        };

        let mut filters: HashMap<ChannelIdentifier, Vec<Filter>> = HashMap::new();

        for filter in db.get_all_filters().expect("DB Error") {
            let channel = db
                .get_channel_by_id(filter.channel_id)
                .expect("DB error")
                .unwrap(); // None is mpossible because channel_id is a foreign key
            let channel_identifier = channel.get_identifier();

            if let Some(channel_filters) = filters.get_mut(&channel_identifier) {
                channel_filters.push(filter);
            } else {
                filters.insert(channel_identifier, vec![filter]);
            }
        }

        tracing::trace!("Loaded filters: {:?}", filters);

        let minecraft = match minecraft::init() {
            Ok(mut minecraft) => {
                db.get_or_create_channel(&ChannelIdentifier::Minecraft)
                    .expect("DB error")
                    .expect("Failed to initialize Minecraft channel in the DB!");
                if env::var("PROFILE") == Ok("debug".to_string()) {
                    minecraft
                        .send_command("say Foobot2 connected".to_string())
                        .unwrap();
                }
                Some(minecraft)
            }
            Err(e) => {
                tracing::error!("Failed to initialize Minecraft: {}", e);
                None
            }
        };

        let platform_handler = PlatformHandler {
            twitch_api,
            discord_api,
            irc_sender: None,
            minecraft_client: minecraft.and_then(|m| Some(Arc::new(Mutex::new(m)))),
            filters: Arc::new(std::sync::RwLock::new(filters)),
        };

        let lingva_api = LingvaApi::init(lingva_url);

        let mut template_registry = Handlebars::new();

        template_registry.register_helper("translate", Box::new(lingva_api));
        template_registry.register_helper("args", Box::new(inquiry_helper::args_helper));
        template_registry.register_helper("spotify", Box::new(SpotifyHelper { db: db.clone() }));
        template_registry.register_helper(
            "spotify_last_song",
            Box::new(SpotifyLastHelper { db: db.clone() }),
        );
        template_registry.register_helper(
            "spotify_playlist",
            Box::new(SpotifyPlaylistHelper { db: db.clone() }),
        );
        template_registry.register_helper("choose", Box::new(random_helper));
        template_registry.register_helper("sleep", Box::new(sleep_helper));
        template_registry.register_helper("username", Box::new(username_helper));
        template_registry.register_helper("concat", Box::new(concat_helper));
        template_registry.register_helper("trim_matches", Box::new(trim_matches_helper));

        if let Ok(api_key) = env::var("FINNHUB_API_KEY") {
            template_registry.register_helper("stock", Box::new(FinnhubApi::init(api_key)));
        }

        if let Ok(owm_api_key) = env::var("OWM_API_KEY") {
            template_registry.register_helper(
                "weather",
                Box::new(WeatherHelper {
                    db: db.clone(),
                    api: OwmApi::init(owm_api_key),
                }),
            );
        }

        if let Ok(lastfm_api_key) = env::var("LASTFM_API_KEY") {
            template_registry.register_helper(
                "lastfm",
                Box::new(LastFMHelper {
                    db: db.clone(),
                    lastfm_api: LastFMApi::init(lastfm_api_key),
                }),
            )
        }

        if let Some(twitch_api) = &platform_handler.twitch_api {
            template_registry.register_helper(
                "twitchuser",
                Box::new(TwitchUserHelper {
                    twitch_api: twitch_api.clone(),
                }),
            );
            template_registry.register_helper(
                "twitch_commercial",
                Box::new(CommercialHelper { db: db.clone() }),
            );
        }

        template_registry.register_helper("get", Box::new(HttpHelper::init()));

        template_registry.register_helper("json", Box::new(JsonHelper));

        template_registry.register_helper("song", Box::new(inquiry_helper::song_helper));

        let temp_data = Arc::new(DashMap::new());

        template_registry.register_helper(
            "data_get",
            Box::new(GetTempData {
                data: temp_data.clone(),
            }),
        );

        template_registry.register_helper(
            "say",
            Box::new(inquiry_helper::SayHelper {
                platform_handler: platform_handler.clone(),
            }),
        );

        template_registry.register_helper("data_set", Box::new(SetTempData { data: temp_data }));

        template_registry.register_decorator("set", Box::new(set_decorator));

        template_registry.register_helper("rhai", Box::new(script::RhaiHelper::default()));

        template_registry.set_strict_mode(true);

        let cooldowns = Arc::new(RwLock::new(Vec::new()));

        let mut mirror_connections = HashMap::new();
        for connection in db.get_mirror_connections().expect("DB error") {
            let from_channel = db
                .get_channel_by_id(connection.from_channel_id)
                .unwrap()
                .expect("Invalid channel connection");

            let to_channel = db
                .get_channel_by_id(connection.to_channel_id)
                .unwrap()
                .expect("Invalid channel connection");

            if let Some(from_channel_str) = from_channel.get_identifier().get_channel() {
                mirror_connections.insert(
                    format!("{}-{}", from_channel.platform, from_channel_str),
                    to_channel.get_identifier(),
                );
            }
        }
        tracing::info!("Mirroring channels: {:?}", mirror_connections);

        start_supinic_heartbeat().await;

        Self {
            db,
            platform_handler: Arc::new(RwLock::new(platform_handler)),
            startup_time: Arc::new(Instant::now()),
            template_registry: Arc::new(template_registry),
            cooldowns,
            mirror_connections: Arc::new(mirror_connections),
            command_triggers: Arc::new(DashMap::new()),
        }
    }

    pub async fn handle_message<C>(&self, message_text: &str, context: C) -> Option<String>
    where
        C: ExecutionContext + Sync,
    {
        let channel = context.get_channel();
        let platform_handler = self.platform_handler.read().await;

        self.handle_message_internal(message_text, context)
            .await
            .and_then(|mut response| {
                platform_handler.filter_message(&mut response, &channel);

                if !response.is_empty() {
                    Some(response)
                } else {
                    None
                }
            })
    }

    pub async fn handle_message_internal<C>(&self, message_text: &str, context: C) -> Option<String>
    where
        C: ExecutionContext + Sync,
    {
        tracing::trace!("Handling message in channel {}", context.get_channel());
        if let Some(mirror_channel) = self.mirror_connections.get(&format!(
            "{}-{}",
            context
                .get_channel()
                .get_platform_name()
                .unwrap_or_default(),
            context.get_channel().get_channel().unwrap_or_default()
        )) {
            let platform_handler = self.platform_handler.clone();
            let mirror_channel = mirror_channel.clone();
            let channel = context.get_channel();
            let display_name = context.get_display_name().to_string();

            let msg = format!("[{}] {}: {}", channel, display_name, message_text);
            tracing::info!(
                "Mirroring message from {} to {}: {}",
                context.get_channel(),
                mirror_channel,
                msg
            );
            // TODO
            if display_name != "egsbot" {
                tokio::spawn(async move {
                    let platform_handler = platform_handler.read().await;
                    if let Err(e) = platform_handler.send_to_channel(mirror_channel, msg).await {
                        tracing::warn!("Failed to mirror message: {}", e);
                    }
                });
            }
        }

        if let Some(channel) = self
            .db
            .get_or_create_channel(&context.get_channel())
            .expect("DB error")
        {
            let triggers = self.get_command_triggers(channel.id).expect("DB error");

            for trigger in triggers.iter() {
                if let Some(command_args) = message_text.strip_prefix(trigger.key()) {
                    let command_msg = format!("{} {}", trigger.value(), command_args);
                    tracing::info!("Executing indirect command {}", command_msg);

                    return self.handle_command_message(&command_msg, context).await;
                }
            }
        }

        for prefix in context.get_prefixes() {
            if let Some(command_msg) = message_text.strip_prefix(prefix) {
                return self.handle_command_message(command_msg, context).await;
            }
        }
        None
    }

    /// This function expects a raw message that appears to be a command without the leading command prefix.
    async fn handle_command_message<C>(&self, message_text: &str, context: C) -> Option<String>
    where
        C: ExecutionContext + Sync,
    {
        if !message_text.trim().is_empty() {
            let mut split = message_text.split_whitespace();
            let command_string = split.next().unwrap().to_owned();
            let arguments: Vec<&str> = split.collect();

            let command_result = match BuiltinCommand::from_str(&command_string) {
                Ok(builtin) => self.run_command(builtin, arguments, context).await,
                Err(()) => {
                    let custom_command = self
                        .db
                        .get_command(&context.get_channel(), &command_string)
                        .expect("DB error")?;

                    self.run_command(custom_command, arguments, context).await
                }
            };
            match command_result {
                Ok(result) if !result.is_empty() => Some(result),
                Err(e) => Some(e.to_string()),
                _ => None,
            }
        } else {
            None
        }
    }

    async fn run_command<C: ExecutionContext + std::marker::Sync, X: Command>(
        &self,
        command: X,
        arguments: Vec<&str>,
        execution_context: C,
    ) -> Result<String, CommandError> {
        tracing::info!("Processing command {} with {:?}", command, arguments);

        let user_identifier = execution_context.get_user_identifier();
        let user = self.db.get_or_create_user(&user_identifier)?;
        let command_string = command.to_string();

        if !self
            .cooldowns
            .read()
            .await
            .contains(&(user.id, command_string.clone()))
        {
            let permissions = command.get_permissions();
            let cooldown = command.get_cooldown();

            self.start_cooldown(user.id, command_string, cooldown).await;

            // This optimization is needed to avoid quering permissions, since it can be expensive on some platforms.
            let permitted = if permissions >= Permissions::Default {
                execution_context.get_permissions().await >= permissions
            } else {
                true
            };

            if permitted {
                let response = command
                    .execute(self, arguments, &execution_context, &user)
                    .await?;

                Ok(response)
            } else {
                Err(CommandError::NoPermissions)
            }

            /*let result = match command {
                // "ping" => (Some(self.ping().await), Some(5)),
                "whoami" | "id" => (
                    Some(format!(
                        "{:?}, identified as {}, channel: {}, permissions: {:?}",
                        user,
                        user_identifier,
                        execution_context.get_channel(),
                        execution_context.get_permissions().await,
                    )),
                    Some(5),
                ),
                "help" => (
                    self.edit_cmds("commands", vec![], execution_context)
                        .await?,
                    Some(5),
                ),
                "cmd" => (
                    self.edit_cmds(command, arguments, execution_context)
                        .await?,
                    Some(1),
                ),
                "command" | "commands" => (
                    self.edit_cmds(command, vec![], execution_context).await?,
                    Some(5),
                ),
                // Old commands for convenience
                "addcmd" | "cmdadd" => (
                    self.edit_cmds(
                        "command",
                        {
                            arguments.insert(0, "add");
                            arguments
                        },
                        execution_context,
                    )
                    .await?,
                    Some(1),
                ),
                "delcmd" | "cmddel" => (
                    self.edit_cmds(
                        "command",
                        {
                            arguments.insert(0, "remove");
                            arguments
                        },
                        execution_context,
                    )
                    .await?,
                    Some(1),
                ),
                "showcmd" | "checkcmd" => (
                    self.edit_cmds(
                        "command",
                        {
                            arguments.insert(0, "show");
                            arguments
                        },
                        execution_context,
                    )
                    .await?,
                    Some(1),
                ),
                "debug" | "check" => {
                    if execution_context.get_permissions().await >= Permissions::ChannelMod {
                        let action = arguments.join(" ");

                        (
                            self.execute_command_action(
                                action,
                                execution_context,
                                user.clone(),
                                arguments.into_iter().map(|a| a.to_owned()).collect(),
                            )
                            .await?,
                            None,
                        )
                    } else {
                        (
                            Some("Debug is only available to mods and higher!".to_owned()),
                            Some(5),
                        )
                    }
                }
                "sh" | "shell" => {
                    let allow_shell =
                        env::var("ALLOW_SHELL").map_err(|_| CommandError::NoPermissions)?;

                    match &allow_shell as &str {
                        "1" => match execution_context.get_permissions().await {
                            Permissions::Admin => {
                                let mut cmd = Command::new("sh");

                                cmd.arg("-c").arg(arguments.join(" "));

                                tracing::info!("Running command {:?}", cmd);

                                let output = cmd
                                    .output()
                                    .await
                                    .map(|o| {
                                        let stdout =
                                            String::from_utf8_lossy(&o.stdout).into_owned();
                                        let stderr =
                                            String::from_utf8_lossy(&o.stderr).into_owned();
                                        format!("{}{}", stdout, stderr)
                                    })
                                    .unwrap_or_else(|e| e.to_string())
                                    .trim()
                                    .to_owned();

                                (Some(output), None)
                            }
                            _ => return Err(CommandError::NoPermissions),
                        },
                        _ => return Err(CommandError::NoPermissions),
                    }
                }
                "eventsub" => {
                    let mut arguments = arguments.into_iter();

                    let action = arguments.next().context("action not specified")?;

                    (
                        Some(
                            self.manage_eventsub(action, arguments, execution_context)
                                .await?,
                        ),
                        None,
                    )
                }
            };*/
        } else {
            tracing::info!("Ignoring command, on cooldown");
            Ok(String::new())
        }
    }

    async fn start_cooldown(&self, user_id: u64, command: String, cooldown: u64) {
        let cooldowns = self.cooldowns.clone();
        task::spawn(async move {
            {
                let mut cooldowns = cooldowns.write().await;
                cooldowns.push((user_id, command.clone()));
            }
            tokio::time::sleep(Duration::from_secs(cooldown)).await;
            {
                let mut cooldowns = cooldowns.write().await;
                tracing::debug!("{:?}", cooldowns);
                cooldowns.retain(|(id, cmd)| id != &user_id && cmd != &command)
            }
        });
    }

    async fn execute_command_action<C: ExecutionContext>(
        &self,
        action: String,
        execution_context: &C,
        user: User,
        arguments: Vec<String>,
    ) -> Result<String, CommandError> {
        tracing::info!("Parsing action {}", action);

        let template_registry = self.template_registry.clone();

        let display_name = execution_context.get_display_name().to_string();
        let channel = execution_context.get_channel();

        Ok(task::spawn_blocking(move || {
            template_registry.render_template(
                &action,
                &(InquiryContext {
                    user,
                    arguments: arguments.iter().map(|s| s.to_owned()).collect(),
                    display_name,
                    channel,
                }),
            )
        })
        .await
        .expect("Failed to join")?)
    }

    async fn manage_eventsub<C: ExecutionContext + Sync>(
        &self,
        action: &str,
        arguments: IntoIter<&str>,
        execution_context: C,
    ) -> anyhow::Result<String> {
        if execution_context.get_permissions().await < Permissions::ChannelMod {
            return Err(CommandError::NoPermissions.into());
        }

        if let ChannelIdentifier::TwitchChannel((broadcaster_id, _)) =
            execution_context.get_channel()
        {
            let platform_handler = self.platform_handler.read().await;

            let app_api = &platform_handler.twitch_api.as_ref().unwrap().helix_api_app;

            match action {
                "add" | "create" => {
                    let (subscription, action) = self
                        .get_subscription(arguments, broadcaster_id.clone())
                        .await?;

                    if action.is_empty() {
                        return Err(anyhow!("Action not specified"));
                    }

                    let subscription_response = app_api
                        .add_eventsub_subscription(subscription.clone())
                        .await
                        .map_err(|e| anyhow!("Failed to create subscription: {}", e))?;

                    let id = &subscription_response.data.first().unwrap().id;

                    self.db.add_eventsub_trigger(NewEventSubTrigger {
                        broadcaster_id: &broadcaster_id,
                        event_type: subscription.get_type(),
                        action: &action,
                        creation_payload: &serde_json::to_string(&subscription)?,
                        id,
                    })?;

                    Ok("Trigger successfully added".to_string())
                }
                "remove" | "delete" => {
                    let (subscription_type, _) = self
                        .get_subscription(arguments, broadcaster_id.clone())
                        .await?;

                    let subscriptions = app_api
                        .get_eventsub_subscriptions(Some(subscription_type.get_type()))
                        .await?;

                    for subscription in subscriptions {
                        if subscription.condition == subscription_type.get_condition() {
                            app_api
                                .delete_eventsub_subscription(&subscription.id)
                                .await?;

                            self.db.delete_eventsub_trigger(&subscription.id)?;

                            return Ok("Trigger succesfully removed".to_string());
                        }
                    }

                    return Err(anyhow!("Unable to find matching subscription"));
                }
                "list" => {
                    let triggers = self
                        .db
                        .get_eventsub_triggers_for_broadcaster(&broadcaster_id)?;

                    if !triggers.is_empty() {
                        Ok(triggers
                            .iter()
                            .map(|trigger| trigger.event_type.clone())
                            .collect::<Vec<String>>()
                            .join(", "))
                    } else {
                        Ok("No eventsub triggers registered".to_string())
                    }
                }
                _ => Err(anyhow!("invalid action {}", action)),
            }
        } else {
            Err(anyhow!("EventSub can only be used on Twitch"))
        }
    }
    async fn get_subscription(
        &self,
        mut arguments: IntoIter<&str>,
        broadcaster_id: String,
    ) -> anyhow::Result<(EventSubSubscriptionType, String)> {
        let sub_type = arguments.next().context("Missing subscription type")?;

        let mut action = arguments.collect::<Vec<&str>>().join(" ");

        let subscription = match sub_type {
            "channel.update" => EventSubSubscriptionType::ChannelUpdate(ChannelUpdateCondition {
                broadcaster_user_id: broadcaster_id.clone(),
            }),
            "channel.channel_points_custom_reward_redemption.add" | "points.redeem" => {
                let action_clone = action.clone();

                let (reward_name, action_str) = match action_clone.split_once(";") {
                    Some((reward_name, action_str)) => (reward_name, action_str),
                    None => (action_clone.as_str(), ""),
                };

                tracing::info!("Searching for reward {}", reward_name);

                action = action_str.trim().to_string();
                let reward_name = reward_name.trim();

                let streamer_credentials = self.db.make_twitch_credentials(broadcaster_id.clone());
                let refreshing_credentials = RefreshingLoginCredentials::init(
                    get_client_id().unwrap(),
                    get_client_secret().unwrap(),
                    streamer_credentials,
                );

                refreshing_credentials
                    .get_credentials()
                    .await
                    .context("Streamer not authenticated to manage channel points!")?;

                let streamer_api = HelixApi::with_credentials(refreshing_credentials).await;

                let rewards_response = streamer_api.get_custom_rewards().await?;

                let reward = rewards_response
                    .data
                    .iter()
                    .find(|reward| reward.title.trim() == reward_name)
                    .with_context(|| format!("Failed to find reward \"{}\"", reward_name))?;

                EventSubSubscriptionType::ChannelPointsCustomRewardRedemptionAdd(
                    ChannelPointsCustomRewardRedemptionAddCondition {
                        broadcaster_user_id: broadcaster_id,
                        reward_id: Some(reward.id.clone()),
                    },
                )
            }
            _ => return Err(anyhow!("Invalid subscription type {}", sub_type)),
        };

        Ok((subscription, action))
    }

    async fn edit_cmds<C: ExecutionContext + Sync>(
        &self,
        command: &str,
        arguments: Vec<&str>,
        execution_context: C,
    ) -> Result<Option<String>, CommandError> {
        let channel = self
            .db
            .get_or_create_channel(&execution_context.get_channel())?
            .ok_or_else(|| CommandError::NoPermissions)?; // Shouldn't happen anyway

        let mut arguments = arguments.into_iter();

        let response = if arguments.len() == 0 {
            Ok(Some(format!(
                "{}/channels/{}/commands",
                api::get_base_url(),
                self.db
                    .get_or_create_channel(&execution_context.get_channel())?
                    .ok_or_else(|| CommandError::InvalidArgument(
                        "can't add commands outside of channels".to_string()
                    ))?
                    .id
            )))
        } else if execution_context.get_permissions().await >= Permissions::ChannelMod {
            match arguments.next().ok_or_else(|| {
                CommandError::MissingArgument("must be either add or delete".to_string())
            })? {
                "add" | "create" => {
                    let mut command_name = arguments
                        .next()
                        .ok_or_else(|| CommandError::MissingArgument("command name".to_string()))?;

                    for prefix in execution_context.get_prefixes() {
                        if let Some(stripped_name) = command_name.strip_prefix(prefix) {
                            command_name = stripped_name;
                        }
                    }

                    let command_action = arguments.collect::<Vec<&str>>().join(" ");

                    if command_action.is_empty() {
                        return Err(CommandError::MissingArgument("command action".to_string()));
                    }

                    match self.db.add_command_to_channel(
                        &execution_context.get_channel(),
                        command_name,
                        &command_action,
                    ) {
                        Ok(()) => Ok(Some("Command successfully added".to_string())),
                        Err(DatabaseError::DieselError(diesel::result::Error::DatabaseError(
                            diesel::result::DatabaseErrorKind::UniqueViolation,
                            _,
                        ))) => Ok(Some("Command already exists".to_string())),
                        Err(e) => Err(CommandError::DatabaseError(e)),
                    }
                }
                "del" | "delete" | "remove" => {
                    let mut command_name = arguments
                        .next()
                        .ok_or_else(|| CommandError::MissingArgument("command name".to_string()))?;

                    if let Some(stripped_name) = command_name.strip_prefix('!') {
                        command_name = stripped_name;
                    }

                    match self
                        .db
                        .delete_command_from_channel(&execution_context.get_channel(), command_name)
                    {
                        Ok(()) => Ok(Some("Command succesfully removed".to_string())),
                        Err(e) => Err(CommandError::DatabaseError(e)),
                    }
                }
                "show" | "check" => {
                    let mut command_name = arguments
                        .next()
                        .ok_or_else(|| CommandError::MissingArgument("command name".to_string()))?;

                    if let Some(stripped_name) = command_name.strip_prefix('!') {
                        command_name = stripped_name;
                    }

                    match self
                        .db
                        .get_command(&execution_context.get_channel(), command_name)?
                    {
                        Some(command) => Ok(Some(command.action)),
                        None => Ok(Some(format!("command {} doesn't exist", command_name))),
                    }
                }
                "set_triggers" => {
                    let mut command_name = arguments
                        .next()
                        .ok_or_else(|| CommandError::MissingArgument("command name".to_string()))?;

                    if let Some(stripped_name) = command_name.strip_prefix('!') {
                        command_name = stripped_name;
                    }

                    let triggers = arguments.collect::<Vec<&str>>().join(" ");

                    if triggers.is_empty() {
                        return Err(CommandError::MissingArgument("triggers".to_string()));
                    }

                    self.db
                        .set_command_triggers(channel.id, &command_name, &triggers)?;

                    Ok(Some(String::from("Succesfully updated command triggers")))
                }
                "get_triggers" => {
                    let mut command_name = arguments
                        .next()
                        .ok_or_else(|| CommandError::MissingArgument("command name".to_string()))?;

                    if let Some(stripped_name) = command_name.strip_prefix('!') {
                        command_name = stripped_name;
                    }

                    let commands = self.db.get_commands(channel.id)?;

                    for command in commands {
                        if command.name == command_name {
                            return Ok(match command.triggers {
                                Some(triggers) => Some(format!("Command triggers: {}", triggers)),
                                None => Some(String::from("Command has no triggers")),
                            });
                        }
                    }
                    Ok(Some(String::from("Command not found")))
                }
                _ => Err(CommandError::InvalidArgument(command.to_string())),
            }
        } else {
            Err(CommandError::NoPermissions)
        }?;

        self.refresh_command_triggers(channel.id)?;

        Ok(response)
    }

    pub async fn get_permissions_in_channel(
        &self,
        user: User,
        channel: &ChannelIdentifier,
    ) -> anyhow::Result<Permissions> {
        if let Ok(Some(admin_user)) = self.db.get_admin_user() {
            if user.id == admin_user.id {
                return Ok(Permissions::Admin);
            }
        }

        match channel {
            ChannelIdentifier::TwitchChannel((channel_id, _)) => {
                let twitch_id = user
                    .twitch_id
                    .ok_or_else(|| anyhow!("Not registered on this platform"))?;

                let platform_handler = self.platform_handler.read().await;

                let twitch_api = platform_handler
                    .twitch_api
                    .as_ref()
                    .ok_or_else(|| anyhow!("Twitch not configured"))?;

                let users_response = twitch_api
                    .helix_api
                    .get_users(None, Some(&vec![channel_id]))
                    .await?;

                let channel_login = &users_response.first().expect("User not found").login;

                match twitch_api.get_channel_mods(channel_login).await?.contains(
                    &twitch_api
                        .helix_api
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
            ChannelIdentifier::DiscordChannel(guild_id) => {
                let user_id = user
                    .discord_id
                    .ok_or_else(|| anyhow!("Invalid user"))?
                    .parse()
                    .unwrap();

                let platform_handler = self.platform_handler.read().await;
                let discord_api = platform_handler.discord_api.as_ref().unwrap();

                match discord_api
                    .get_permissions_in_guild(user_id, guild_id.parse().unwrap())
                    .await
                    .map_err(|_| anyhow!("discord error"))?
                    .contains(twilight_model::guild::Permissions::ADMINISTRATOR)
                {
                    true => Ok(Permissions::ChannelMod),
                    false => Ok(Permissions::Default),
                }
            }
            ChannelIdentifier::IrcChannel(_) => Ok(Permissions::Default), // TODO
            ChannelIdentifier::Anonymous => Ok(Permissions::Default),
            ChannelIdentifier::LocalAddress(_) => Ok(Permissions::ChannelOwner), // on the local platform, each ip address is its own channel
            ChannelIdentifier::Minecraft => Ok(Permissions::Default),
            ChannelIdentifier::TelegramChat(_) => Ok(Permissions::Default), // TODO
        }
    }

    pub async fn get_permissions_in_channel_by_id(
        &self,
        user_id: u64,
        channel_id: u64,
    ) -> anyhow::Result<Permissions> {
        let user = self
            .db
            .get_user_by_id(user_id)?
            .ok_or_else(|| anyhow!("Invalid user id"))?;

        match self.db.get_channel_by_id(channel_id)? {
            Some(channel) => {
                let channel_identifier =
                    ChannelIdentifier::new(&channel.platform, channel.channel)?;

                self.get_permissions_in_channel(user, &channel_identifier)
                    .await
            }

            None => Ok(Permissions::Default),
        }
    }

    pub async fn handle_server_message(
        &self,
        action: String,
        execution_context: ServerExecutionContext,
        arguments: Vec<String>,
    ) -> anyhow::Result<()> {
        let user = self
            .db
            .get_or_create_user(&execution_context.executing_user)?;

        let response = self
            .execute_command_action(action, &execution_context, user, arguments) // TODO
            .await?;

        Ok(self
            .platform_handler
            .read()
            .await
            .send_to_channel(execution_context.get_channel(), response)
            .await?)
    }

    pub async fn join_channel(&self, channel: &ChannelIdentifier) -> anyhow::Result<()> {
        match channel {
            ChannelIdentifier::TwitchChannel((id, _)) => {
                let platform_handler = self.platform_handler.read().await;
                let twitch_api = platform_handler
                    .twitch_api
                    .as_ref()
                    .context("Twitch not initialized")?;

                let user = twitch_api.helix_api.get_user_by_id(id).await?;

                let chat_sender_guard = twitch_api.chat_sender.lock().await;
                let chat_sender = chat_sender_guard
                    .as_ref()
                    .context("Twitch chat not initialized")?;

                chat_sender
                    .send(twitch::SenderMessage::JoinChannel(user.login.clone()))
                    .unwrap();
                chat_sender
                    .send(twitch::SenderMessage::Privmsg(twitch::Privmsg {
                        channel_login: user.login,
                        message: String::from("MrDestructoid 👍 Foobot2 joined"),
                        reply_to_id: None,
                    }))
                    .unwrap();

                self.db
                    .get_or_create_channel(channel)?
                    .context("Failed to add channel")?;

                Ok(())
            }
            ChannelIdentifier::DiscordChannel(_) => Ok(()), // Discord guilds don't need to be joined client side and get added to the DB on demand
            ChannelIdentifier::TelegramChat(_) => todo!(),
            ChannelIdentifier::IrcChannel(_) => Err(anyhow!("Not implemented yet")),
            ChannelIdentifier::LocalAddress(_) => Err(anyhow!("This is not possible")),
            ChannelIdentifier::Minecraft => panic!("This should never happen"),
            ChannelIdentifier::Anonymous => Err(anyhow!("Invalid channel specified")),
        }
    }

    fn get_command_triggers(
        &self,
        channel_id: u64,
    ) -> Result<Arc<DashMap<String, String>>, CommandError> {
        match self.command_triggers.get(&channel_id) {
            Some(triggers) => Ok(triggers.clone()),
            None => {
                self.refresh_command_triggers(channel_id)?;
                self.get_command_triggers(channel_id)
            }
        }
    }

    fn refresh_command_triggers(&self, channel_id: u64) -> Result<(), CommandError> {
        let commands = self.db.get_commands(channel_id)?;

        let triggers = DashMap::new();

        for command in commands {
            if let Some(command_triggers) = command.triggers {
                for trigger in command_triggers.split(";") {
                    triggers.insert(trigger.to_string(), command.name.clone());
                }
            }
        }

        if let Some(_) = self.command_triggers.insert(channel_id, Arc::new(triggers)) {
            tracing::info!("Reloaded command triggers in channel {}", channel_id);
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    MissingArgument(String),
    InvalidArgument(String),
    NoPermissions,
    DatabaseError(DatabaseError),
    TemplateError(handlebars::RenderError),
    ConfigurationError(VarError),
    GenericError(String),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::MissingArgument(arg) => {
                f.write_str(&format!("missing argument: {}", arg))
            }
            CommandError::InvalidArgument(arg) => {
                f.write_str(&format!("invalid argument: {}", arg))
            }
            CommandError::NoPermissions => {
                f.write_str("you don't have the permissions to use this command")
            }
            CommandError::DatabaseError(e) => f.write_str(&e.to_string()),
            CommandError::TemplateError(e) => f.write_str(&e.to_string()),
            CommandError::ConfigurationError(e) => {
                f.write_str(&format!("configuration error: {}", e))
            }
            CommandError::GenericError(s) => f.write_str(s),
        }
    }
}

impl From<diesel::result::Error> for CommandError {
    fn from(e: diesel::result::Error) -> Self {
        Self::DatabaseError(DatabaseError::DieselError(e))
    }
}

impl From<DatabaseError> for CommandError {
    fn from(e: DatabaseError) -> Self {
        Self::DatabaseError(e)
    }
}

impl From<handlebars::RenderError> for CommandError {
    fn from(e: handlebars::RenderError) -> Self {
        Self::TemplateError(e)
    }
}

impl From<VarError> for CommandError {
    fn from(e: VarError) -> Self {
        Self::ConfigurationError(e)
    }
}

impl From<ParseIntError> for CommandError {
    fn from(_: ParseIntError) -> Self {
        Self::InvalidArgument("expected a number".to_string())
    }
}

impl From<UserIdentifierError> for CommandError {
    fn from(e: UserIdentifierError) -> Self {
        match e {
            UserIdentifierError::MissingDelimiter => Self::MissingArgument(
                "separator `:`! Must be in the form of `platform:user`".to_string(),
            ),
            UserIdentifierError::InvalidPlatform => Self::InvalidArgument("platform".to_string()),
        }
    }
}

impl From<anyhow::Error> for CommandError {
    fn from(e: anyhow::Error) -> Self {
        Self::GenericError(e.to_string())
    }
}

async fn start_supinic_heartbeat() {
    task::spawn(async move {
        let client = Client::new();

        let user_id = env::var("SUPINIC_USER_ID").unwrap_or_default();
        let pass = env::var("SUPINIC_PASSWORD").unwrap_or_default();

        loop {
            tracing::info!("Pinging Supinic API");

            match client
                .put("https://supinic.com/api/bot-program/bot/active")
                .header("Authorization", format!("Basic {}:{}", user_id, pass))
                .send()
                .await
            {
                Ok(response) => {
                    if !response.status().is_success() {
                        let text = response.text().await;
                        tracing::info!("Supinic API error: {:?}", text);
                    }
                }
                Err(e) => tracing::warn!("Failed to ping Supinic API! {}", e),
            }

            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });
}

pub fn get_admin_channel() -> Option<ChannelIdentifier> {
    if let Ok(admin_str) = env::var("ADMIN_USER") {
        match ChannelIdentifier::from_str(&admin_str) {
            Ok(admin_channel) => Some(admin_channel),
            Err(e) => {
                tracing::warn!("Failed to get admin channel: {}", e);
                None
            }
        }
    } else {
        None
    }
}
