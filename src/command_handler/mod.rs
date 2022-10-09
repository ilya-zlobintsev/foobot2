mod commands;
pub mod discord_api;
mod error;
pub mod finnhub_api;
pub mod inquiry_helper;
pub mod lastfm_api;
pub mod lingva_api;
pub mod owm_api;
pub mod platform_handler;
pub mod spotify_api;
pub mod twitch_api;

use anyhow::anyhow;
use dashmap::DashMap;
use discord_api::DiscordApi;
use handlebars::Handlebars;
use inquiry_helper::*;
use lastfm_api::LastFMApi;
use lingva_api::LingvaApi;
use once_cell::sync::Lazy;
use owm_api::OwmApi;
use prometheus::{HistogramOpts, HistogramVec, IntCounterVec, Opts};
use reqwest::Client;
use std::collections::HashMap;
use std::env;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::task;
use twitch_api::TwitchApi;

use self::commands::BuiltinCommand;
use self::error::CommandError;
use self::finnhub_api::FinnhubApi;
use self::platform_handler::PlatformHandler;
use crate::command_handler::commands::{create_builtin_commands, ExecutableCommand};
use crate::database::models::Filter;
use crate::database::{models::User, Database};
use crate::platform::minecraft;
use crate::platform::{ChannelIdentifier, Permissions, PlatformContext, ServerPlatformContext};

const DEFAULT_COOLDOWN: u64 = 5;

pub static COMMAND_COUNTER: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new("command_counter", "Command call counter"),
        &["command", "channel", "success"],
    )
    .expect("Failed to create counter")
});

pub static COMMAND_PROCESSING_HISTOGRAM: Lazy<HistogramVec> = Lazy::new(|| {
    HistogramVec::new(
        HistogramOpts::new(
            "command_processing_duration",
            "The time it took to process a command",
        ),
        &["command"],
    )
    .expect("Failed to create histogram")
});

#[derive(Clone)]
pub struct CommandHandler {
    pub db: Database,
    pub platform_handler: Arc<RwLock<PlatformHandler>>,
    template_registry: Arc<Handlebars<'static>>,
    builtin_commands: Arc<Vec<BuiltinCommand>>,
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
            minecraft_client: minecraft.map(|m| Arc::new(Mutex::new(m))),
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
        template_registry.register_helper(
            "forsencode_encode",
            Box::new(inquiry_helper::forsencode_encode_helper),
        );
        template_registry.register_helper(
            "forsencode_decode",
            Box::new(inquiry_helper::forsencode_decode_helper),
        );

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

        let platform_handler = Arc::new(RwLock::new(platform_handler));

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

        let template_registry = Arc::new(template_registry);

        let builtin_commands = create_builtin_commands(template_registry.clone());
        info!("Loaded builtin commands: {builtin_commands:?}");

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
            platform_handler,
            template_registry,
            cooldowns,
            mirror_connections: Arc::new(mirror_connections),
            command_triggers: Arc::new(DashMap::new()),
            builtin_commands: Arc::new(builtin_commands),
        }
    }

    pub async fn handle_message<P: PlatformContext + Send + Sync>(
        &self,
        message_text: &str,
        platform_ctx: P,
    ) -> Option<String> {
        let channel = platform_ctx.get_channel();
        let platform_handler = self.platform_handler.read().await;

        self.handle_message_internal(message_text, platform_ctx)
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

    pub async fn handle_message_internal<P: PlatformContext + Send + Sync>(
        &self,
        message_text: &str,
        platform_ctx: P,
    ) -> Option<String> {
        tracing::trace!("Handling message in channel {}", platform_ctx.get_channel());
        if let Some(mirror_channel) = self.mirror_connections.get(&format!(
            "{}-{}",
            platform_ctx
                .get_channel()
                .get_platform_name()
                .unwrap_or_default(),
            platform_ctx.get_channel().get_channel().unwrap_or_default()
        )) {
            let platform_handler = self.platform_handler.clone();
            let mirror_channel = mirror_channel.clone();
            let channel = platform_ctx.get_channel();
            let display_name = platform_ctx.get_display_name().to_string();

            let msg = format!("[{}] {}: {}", channel, display_name, message_text);
            tracing::info!(
                "Mirroring message from {} to {}: {}",
                platform_ctx.get_channel(),
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
            .get_or_create_channel(&platform_ctx.get_channel())
            .expect("DB error")
        {
            let triggers = self.get_command_triggers(channel.id).expect("DB error");

            for trigger in triggers.iter() {
                if let Some(command_args) = message_text.strip_prefix(trigger.key()) {
                    let command_msg = format!("{} {}", trigger.value(), command_args);
                    tracing::info!("Executing indirect command {}", command_msg);

                    return self
                        .handle_command_message(&command_msg, platform_ctx)
                        .await;
                }
            }
        }

        for prefix in platform_ctx.get_prefixes() {
            if let Some(command_msg) = message_text.strip_prefix(prefix) {
                return self.handle_command_message(command_msg, platform_ctx).await;
            }
        }
        None
    }

    /// This function expects a raw message that appears to be a command without the leading command prefix.
    async fn handle_command_message<C>(&self, message_text: &str, context: C) -> Option<String>
    where
        C: PlatformContext + Send + Sync,
    {
        if message_text.trim().is_empty() {
            Some("❗".to_string())
        } else {
            let mut split = message_text.split_whitespace();

            let command = split.next().unwrap().to_owned();

            let arguments: Vec<&str> = split.collect();

            let timer = COMMAND_PROCESSING_HISTOGRAM
                .with_label_values(&[&command])
                .start_timer();
            let channel = context.get_channel().to_string();

            let command_result = self.run_command(&command, arguments, context).await;

            timer.observe_duration();

            match command_result {
                Ok(result) => {
                    if result.is_some() {
                        COMMAND_COUNTER
                            .with_label_values(&[&command, &channel, "true"])
                            .inc();
                    }

                    result
                }
                Err(e) => {
                    COMMAND_COUNTER
                        .with_label_values(&[&command, &channel, "false"])
                        .inc();

                    Some(e.to_string())
                }
            }
        }
    }

    // #[async_recursion]
    async fn run_command<P: PlatformContext + Send + Sync>(
        &self,
        command: &str,
        args: Vec<&str>,
        platform_ctx: P,
    ) -> Result<Option<String>, CommandError> {
        tracing::info!("Processing command {} with {:?}", command, args);

        let user_identifier = platform_ctx.get_user_identifier();
        let user = self.db.get_or_create_user(&user_identifier)?;

        if !self
            .cooldowns
            .read()
            .await
            .contains(&(user.id, command.to_string()))
        {
            let platform_handler = self.platform_handler.read().await;
            let execution_ctx = ExecutionContext {
                db: &self.db,
                platform_handler: &*platform_handler,
                platform_ctx,
                user: &user,
            };

            let (output, cooldown) = if let Some(builtin_command) = self
                .builtin_commands
                .iter()
                .find(|cmd| cmd.get_names().contains(&command))
            {
                let command_permissions = builtin_command.get_permissions();
                if command_permissions > Permissions::Default {
                    if command_permissions > execution_ctx.platform_ctx.get_permissions().await {
                        return Err(CommandError::NoPermissions);
                    }
                }

                let cooldown = builtin_command.get_cooldown();
                let output = builtin_command
                    .execute(&execution_ctx, command, args)
                    .await?;

                (output, cooldown)
            } else if let Some(command) = self
                .db
                .get_command(&execution_ctx.platform_ctx.get_channel(), command)?
            {
                // TODO custom permissions

                let output = execute_command_action(
                    self.template_registry.clone(),
                    command.action,
                    &execution_ctx,
                    args.into_iter().map(|a| a.to_owned()).collect(),
                )
                .await?;

                (output, command.cooldown.unwrap_or(DEFAULT_COOLDOWN))
            } else {
                (None, 0)
            };

            if cooldown != 0 {
                self.start_cooldown(user.id, command.to_string(), cooldown)
                    .await;
            }

            Ok(output)
        } else {
            tracing::debug!("Ignoring command, on cooldown");
            Ok(None)
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
        platform_ctx: ServerPlatformContext,
        arguments: Vec<String>,
    ) -> anyhow::Result<()> {
        let user = self.db.get_or_create_user(&platform_ctx.executing_user)?;

        let platform_handler = self.platform_handler.read().await;
        let execution_ctx = ExecutionContext {
            db: &self.db,
            platform_handler: &*platform_handler,
            platform_ctx,
            user: &user,
        };

        let response = execute_command_action(
            self.template_registry.clone(),
            action,
            &execution_ctx,
            arguments,
        ) // TODO
        .await?
        .unwrap_or_else(|| "Event triggered with no action".to_string());

        Ok(self
            .platform_handler
            .read()
            .await
            .send_to_channel(execution_ctx.platform_ctx.target_channel, response)
            .await?)
    }

    /*pub async fn join_channel(&self, channel: &ChannelIdentifier) -> anyhow::Result<()> {
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
    }*/

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
                for trigger in command_triggers.split(';') {
                    triggers.insert(trigger.to_string(), command.name.clone());
                }
            }
        }

        if self
            .command_triggers
            .insert(channel_id, Arc::new(triggers))
            .is_some()
        {
            tracing::info!("Reloaded command triggers in channel {}", channel_id);
        }

        Ok(())
    }
}

pub struct ExecutionContext<'a, P: PlatformContext> {
    db: &'a Database,
    platform_handler: &'a PlatformHandler,
    platform_ctx: P,
    user: &'a User,
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

async fn execute_command_action<P: PlatformContext>(
    template_registry: Arc<Handlebars<'static>>,
    action: String,
    ctx: &ExecutionContext<'_, P>,
    args: Vec<String>,
) -> Result<Option<String>, CommandError> {
    tracing::debug!("Parsing action {}", action);

    let display_name = ctx.platform_ctx.get_display_name().to_string();
    let channel = ctx.platform_ctx.get_channel();
    let user = ctx.user.clone();

    let response = match task::spawn_blocking(move || {
        template_registry.render_template(
            &action,
            &(InquiryContext {
                user,
                arguments: args,
                display_name,
                channel,
            }),
        )
    })
    .await
    .expect("Failed to join")
    {
        Ok(result) => result,
        Err(err) => err.desc,
    };

    if !response.is_empty() {
        Ok(Some(response))
    } else {
        Ok(None)
    }
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
