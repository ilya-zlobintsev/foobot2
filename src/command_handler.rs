pub mod discord_api;
pub mod inquiry_helper;
pub mod spotify_api;
pub mod twitch_api;

use core::fmt;
use std::{
    env::{self, VarError},
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use crate::{
    database::{models::User, Database},
    platform::{ExecutionContext, Permissions, UserIdentifier, UserIdentifierError},
};

use handlebars::Handlebars;
use inquiry_helper::*;
use serenity::{
    client::{Cache, ClientBuilder},
    http::{CacheHttp, Http},
};
use tokio::task;
use twitch_api::TwitchApi;

#[derive(Clone)]
pub struct CommandHandler {
    pub db: Database,
    pub twitch_api: Option<TwitchApi>,
    pub discord_context: Option<Arc<DiscordContext>>,
    startup_time: Instant,
    template_registry: Arc<Handlebars<'static>>,
    cooldowns: Arc<RwLock<Vec<(u64, String)>>>, // User id and command
}

#[derive(Clone)]
pub struct DiscordContext {
    http: Arc<Http>,
    cache: Arc<Cache>,
}

impl CacheHttp for DiscordContext {
    fn http(&self) -> &Http {
        &self.http
    }
}

impl AsRef<Cache> for DiscordContext {
    fn as_ref(&self) -> &Cache {
        &self.cache
    }
}

impl AsRef<serenity::http::Http> for DiscordContext {
    fn as_ref(&self) -> &Http {
        &self.http
    }
}

impl CommandHandler {
    pub async fn init(db: Database) -> Self {
        let twitch_api = match env::var("TWITCH_OAUTH") {
            Ok(oauth) => match TwitchApi::init(&oauth).await {
                Ok(api) => Some(api),
                Err(_) => None,
            },
            Err(_) => {
                tracing::info!("TWICTH_OAUTH missing! Skipping Twitch initialization");
                None
            }
        };

        let discord_context = match env::var("DISCORD_TOKEN") {
            Ok(token) => {
                let client = ClientBuilder::new(token)
                    .await
                    .expect("Failed to start Discord API");

                let cache_and_http = client.cache_and_http;

                Some(Arc::new(DiscordContext {
                    http: cache_and_http.http.clone(),
                    cache: cache_and_http.cache.clone(),
                }))
            }
            Err(_) => None,
        };

        let startup_time = Instant::now();

        let mut template_registry = Handlebars::new();

        template_registry.register_helper("context", Box::new(ContextHelper {}));
        template_registry.register_helper(
            "spotify",
            Box::new(SpotifyHelper {
                db: db.clone(),
                twitch_api: twitch_api.clone(),
            }),
        );

        template_registry.set_strict_mode(true);

        let cooldowns = Arc::new(RwLock::new(Vec::new()));

        Self {
            db,
            twitch_api,
            startup_time,
            template_registry: Arc::new(template_registry),
            discord_context,
            cooldowns,
        }
    }

    /// This function expects a raw message that appears to be a command without the leading command prefix.
    pub async fn handle_command_message<T>(
        &self,
        message: &T,
        context: ExecutionContext,
        user_identifier: UserIdentifier,
    ) -> Option<String>
    where
        T: Sync + CommandMessage,
    {
        let message_text = message.get_text();

        if message_text.is_empty() {
            Some("❗".to_string())
        } else {
            let mut split = message_text.split_whitespace();

            let command = split.next().unwrap().to_owned();

            let arguments: Vec<&str> = split.collect();

            match self
                .run_command(&command, arguments, context, user_identifier)
                .await
            {
                Ok(result) => result,
                Err(e) => Some(e.to_string()),
            }
        }
    }

    // #[async_recursion]
    async fn run_command(
        &self,
        command: &str,
        arguments: Vec<&str>,
        execution_context: ExecutionContext,
        user_identifier: UserIdentifier,
    ) -> Result<Option<String>, CommandError> {
        tracing::info!("Processing command {} with {:?}", command, arguments);

        let user = self.db.get_or_create_user(user_identifier)?;

        if !self
            .cooldowns
            .read()
            .unwrap()
            .contains(&(user.id, command.to_string()))
        {
            let result = match command {
                "ping" => (Some(self.ping()), Some(5)),
                "whoami" | "id" => (
                    Some(format!(
                        "{:?}, permissions: {:?}",
                        user, execution_context.permissions,
                    )),
                    Some(5),
                ),
                "cmd" | "command" | "commands" => (
                    self.edit_cmds(command, arguments, execution_context)
                        .await?,
                    Some(1),
                ),
                // Old commands for convenience
                "addcmd" | "cmdadd" => (
                    self.edit_cmds(
                        "command",
                        {
                            let mut arguments = arguments;
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
                            let mut arguments = arguments;
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
                            let mut arguments = arguments;
                            arguments.insert(0, "show");
                            arguments
                        },
                        execution_context,
                    )
                    .await?,
                    Some(1),
                ),
                "debug" | "check" | "test" => {
                    let action = arguments.join(" ");

                    (
                        self.execute_command_action(
                            &action,
                            execution_context,
                            user.clone(),
                            &arguments,
                        )?,
                        None,
                    )
                }
                "merge" => {
                    let identifier_string = arguments.first().ok_or_else(|| {
                        CommandError::MissingArgument(
                            "user identifier: must be in the form of `platform:id`".to_string(),
                        )
                    })?;

                    let other_identifier =
                        UserIdentifier::from_string(identifier_string, self.twitch_api.as_ref())
                            .await?;

                    let other = self
                        .db
                        .get_user(&other_identifier)?
                        .ok_or_else(|| UserIdentifierError::InvalidUser)?;

                    self.db.merge_users(user.clone(), other)?;

                    (Some("sucessfully merged users".to_string()), None)
                }
                _ => match self.db.get_command(&execution_context.channel, command)? {
                    Some(cmd) => (
                        self.execute_command_action(
                            &cmd.action,
                            execution_context,
                            user.clone(),
                            &arguments,
                        )?,
                        cmd.cooldown,
                    ),
                    None => (None, None),
                },
            };

            if let Some(cooldown) = result.1 {
                self.start_cooldown(user.id, command.to_string(), cooldown)
                    .await;
            }

            Ok(result.0)
        } else {
            tracing::info!("Ignoring command, on cooldown");
            Ok(None)
        }
    }

    async fn start_cooldown(&self, user_id: u64, command: String, cooldown: u64) {
        let cooldowns = self.cooldowns.clone();
        task::spawn(async move {
            {
                let mut cooldowns = cooldowns.write().unwrap();
                cooldowns.push((user_id, command.clone()));
            }
            tokio::time::sleep(Duration::from_secs(cooldown)).await;
            {
                let mut cooldowns = cooldowns.write().unwrap();
                tracing::debug!("{:?}", cooldowns);
                cooldowns.retain(|(id, cmd)| id != &user_id && cmd != &command)
            }
        });
    }

    fn execute_command_action(
        &self,
        action: &str,
        execution_context: ExecutionContext,
        user: User,
        arguments: &Vec<&str>,
    ) -> Result<Option<String>, CommandError> {
        tracing::info!("Parsing action {}", action);

        let context = InquiryContext {
            user,
            execution_context,
            arguments: arguments.iter().map(|s| s.to_owned().to_owned()).collect(),
        };

        let response = self.template_registry.render_template(action, &context)?;

        if !response.is_empty() {
            Ok(Some(response))
        } else {
            Ok(None)
        }
    }

    fn ping(&self) -> String {
        let uptime = {
            let duration = self.startup_time.elapsed();

            let minutes = (duration.as_secs() / 60) % 60;
            let hours = (duration.as_secs() / 60) / 60;

            let mut result = String::new();

            if hours != 0 {
                result.push_str(&format!("{}h ", hours));
            };

            if minutes != 0 {
                result.push_str(&format!("{}m ", minutes));
            }

            if result.is_empty() {
                result.push_str(&format!("{}s", duration.as_secs()));
            }

            result
        };

        format!("Pong! Uptime {}", uptime)
    }

    async fn edit_cmds(
        &self,
        command: &str,
        arguments: Vec<&str>,
        execution_context: ExecutionContext,
    ) -> Result<Option<String>, CommandError> {
        let mut arguments = arguments.into_iter();

        if arguments.len() == 0 {
            Ok(Some(format!(
                "{}/channels/{}/commands",
                env::var("BASE_URL")?,
                self.db.get_channel(&execution_context.channel)?.id
            )))
        } else {
            match execution_context.permissions {
                Permissions::ChannelMod => {
                    match arguments.next().ok_or_else(|| {
                        CommandError::MissingArgument("must be either add or delete".to_string())
                    })? {
                        "add" | "create" => {
                            let command_name = arguments.next().ok_or_else(|| {
                                CommandError::MissingArgument("command name".to_string())
                            })?;

                            let command_action = arguments.collect::<Vec<&str>>().join(" ");

                            if command_action.is_empty() {
                                return Err(CommandError::MissingArgument(
                                    "command action".to_string(),
                                ));
                            }

                            match self.db.add_command(
                                &execution_context.channel,
                                command_name,
                                &command_action,
                            ) {
                                Ok(()) => Ok(Some("Command successfully added".to_string())),
                                Err(diesel::result::Error::DatabaseError(
                                    diesel::result::DatabaseErrorKind::UniqueViolation,
                                    _,
                                )) => Ok(Some("Command already exists".to_string())),
                                Err(e) => Err(CommandError::DatabaseError(e)),
                            }
                        }
                        "del" | "delete" | "remove" => {
                            let command_name = arguments.next().ok_or_else(|| {
                                CommandError::MissingArgument("command name".to_string())
                            })?;

                            match self
                                .db
                                .delete_command(&execution_context.channel, command_name)
                            {
                                Ok(()) => Ok(Some("Command succesfully removed".to_string())),
                                Err(e) => Err(CommandError::DatabaseError(e)),
                            }
                        }
                        "show" | "check" => {
                            let command_name = arguments.next().ok_or_else(|| {
                                CommandError::MissingArgument("command name".to_string())
                            })?;

                            match self
                                .db
                                .get_command(&execution_context.channel, command_name)?
                            {
                                Some(command) => Ok(Some(command.action)),
                                None => Ok(Some(format!("command {} doesn't exist", command_name))),
                            }
                        }
                        _ => Err(CommandError::InvalidArgument(command.to_string())),
                    }
                }
                Permissions::Default => Err(CommandError::NoPermissions),
            }
        }
    }
}

#[derive(Debug)]
pub enum CommandError {
    MissingArgument(String),
    InvalidArgument(String),
    NoPermissions,
    DatabaseError(diesel::result::Error),
    TemplateError(handlebars::RenderError),
    ConfigurationError(VarError),
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
        }
    }
}

impl From<diesel::result::Error> for CommandError {
    fn from(e: diesel::result::Error) -> Self {
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

impl From<UserIdentifierError> for CommandError {
    fn from(e: UserIdentifierError) -> Self {
        match e {
            UserIdentifierError::MissingDelimiter => Self::MissingArgument(
                "separator `:`! Must be in the form of `platform:user`".to_string(),
            ),
            UserIdentifierError::InvalidPlatform => Self::InvalidArgument("platform".to_string()),
            UserIdentifierError::InvalidUser => {
                Self::InvalidArgument("cannot find user".to_string())
            }
        }
    }
}

pub trait CommandMessage {
    fn get_user_identifier(&self) -> UserIdentifier;

    fn get_text(&self) -> String;
}
