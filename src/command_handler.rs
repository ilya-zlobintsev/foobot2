pub mod discord_api;
pub mod inquiry_helper;
pub mod lastfm_api;
pub mod owm_api;
pub mod spotify_api;
pub mod twitch_api;

use crate::database::DatabaseError;
use crate::database::{models::User, Database};
use crate::platform::{ExecutionContext, Permissions, UserIdentifierError};

use core::fmt;
use std::env::{self, VarError};
use std::num::ParseIntError;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use handlebars::Handlebars;
use inquiry_helper::*;
use tokio::process::Command;
use tokio::task;

use discord_api::DiscordApi;
use lastfm_api::LastFMApi;
use owm_api::OwmApi;
use twitch_api::TwitchApi;

#[derive(Clone, Debug)]
pub struct CommandHandler {
    pub db: Database,
    pub twitch_api: Option<TwitchApi>,
    pub discord_api: Option<DiscordApi>,
    startup_time: Arc<Instant>,
    template_registry: Arc<Handlebars<'static>>,
    cooldowns: Arc<RwLock<Vec<(u64, String)>>>, // User id and command
}

impl CommandHandler {
    pub async fn init(db: Database) -> Self {
        let twitch_api = match TwitchApi::init().await {
            Ok(api) => Some(api),
            Err(e) => {
                tracing::info!("Failed to initialize Twitch API: {}", e);
                None
            }
        };

        let discord_api = match env::var("DISCORD_TOKEN") {
            Ok(token) => Some(DiscordApi::new(token)),
            Err(_) => None,
        };

        let mut template_registry = Handlebars::new();

        template_registry.register_helper("args", Box::new(inquiry_helper::args_helper));
        template_registry.register_helper("spotify", Box::new(SpotifyHelper { db: db.clone() }));
        template_registry.register_helper("choose", Box::new(random_helper));
        template_registry.register_helper("sleep", Box::new(sleep_helper));

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

        if let Some(twitch_api) = &twitch_api {
            template_registry.register_helper(
                "twitchuser",
                Box::new(TwitchUserHelper {
                    twitch_api: twitch_api.clone(),
                }),
            );
        }

        template_registry.register_helper("song", Box::new(inquiry_helper::song_helper));

        template_registry.set_strict_mode(true);

        let cooldowns = Arc::new(RwLock::new(Vec::new()));

        Self {
            db,
            twitch_api,
            startup_time: Arc::new(Instant::now()),
            template_registry: Arc::new(template_registry),
            discord_api,
            cooldowns,
        }
    }

    /// This function expects a raw message that appears to be a command without the leading command prefix.
    pub async fn handle_command_message<C>(&self, message_text: &str, context: C) -> Option<String>
    where
        C: ExecutionContext + Sync,
    {
        if message_text.is_empty() {
            Some("❗".to_string())
        } else {
            let mut split = message_text.split_whitespace();

            let command = split.next().unwrap().to_owned();

            let arguments: Vec<&str> = split.collect();

            match self.run_command(&command, arguments, context).await {
                Ok(result) => result,
                Err(e) => Some(e.to_string()),
            }
        }
    }

    // #[async_recursion]
    async fn run_command<C: ExecutionContext + std::marker::Sync>(
        &self,
        command: &str,
        arguments: Vec<&str>,
        execution_context: C,
    ) -> Result<Option<String>, CommandError> {
        tracing::info!("Processing command {} with {:?}", command, arguments);

        let user_identifier = execution_context.get_user_identifier();

        let user = self.db.get_or_create_user(&user_identifier)?;

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
                        "{:?}, identified as {}, channel: {:?}, permissions: {:?}",
                        user,
                        user_identifier.to_string(),
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
                "cmd" | "command" | "commands" => (
                    self.edit_cmds(command, arguments, execution_context)
                        .await?,
                    Some(1),
                ),
                // Old commands for convenience
                "addcmd" | "cmdadd" => (
                    self.edit_cmds("command", 
                        {
                            let mut arguments = arguments;
                            arguments.insert(0, "add");
                            arguments
                        },
                        execution_context)
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
                "debug" | "check" => {
                    let action = arguments.join(" ");

                    (
                        self.execute_command_action(
                            action,
                            execution_context,
                            user.clone(),
                            &arguments,
                        )?,
                        None,
                    )
                }
                "sh" | "shell" => {
                    let allow_shell =
                        env::var("ALLOW_SHELL").map_err(|_| CommandError::NoPermissions)?;

                    match &allow_shell as &str {
                        "1" => match execution_context.get_permissions().await {
                            Permissions::Admin => {
                                let mut cmd = Command::new("sh");

                                cmd.arg("-c").arg(format!("{}", arguments.join(" ")));

                                tracing::info!("Running command {:?}", cmd);

                                let output = cmd
                                    .output()
                                    .await
                                    .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                                    .unwrap_or_else(|e| e.to_string())
                                    .replace("\n", " ")
                                    .trim()
                                    .to_owned();

                                (Some(output), None)
                            }
                            _ => return Err(CommandError::NoPermissions),
                        },
                        _ => return Err(CommandError::NoPermissions),
                    }
                }
                _ => match self
                    .db
                    .get_command(&execution_context.get_channel(), command)?
                {
                    Some(cmd) => {
                        tracing::info!("Executing custom command {:?}", cmd);

                        (
                            self.execute_command_action(
                                cmd.action,
                                execution_context,
                                user.clone(),
                                &arguments,
                            )?,
                            cmd.cooldown,
                        )
                    }
                    None => {
                        tracing::info!("Command not found");

                        (None, None)
                    }
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

    fn execute_command_action<C: ExecutionContext>(
        &self,
        action: String,
        _execution_context: C,
        user: User,
        arguments: &Vec<&str>,
    ) -> Result<Option<String>, CommandError> {
        tracing::info!("Parsing action {}", action);

        let response = match self.template_registry.render_template(
            &action,
            &(InquiryContext {
                user,
                arguments: arguments.iter().map(|s| s.to_owned().to_owned()).collect(),
            }),
        ) {
            Ok(result) => result,
            Err(e) => e.desc,
        };

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

        let mem = psutil::memory::virtual_memory().unwrap();

        format!(
            "Pong! Version: {}, Uptime {}, RAM usage: {}/{} MiB",
            env!("CARGO_PKG_VERSION"),
            uptime,
            mem.used() / 1024 / 1024,
            mem.total() / 1024 / 1024
        )
    }

    async fn edit_cmds<C: ExecutionContext + Sync>(
        &self,
        command: &str,
        arguments: Vec<&str>,
        execution_context: C,
    ) -> Result<Option<String>, CommandError> {
        let mut arguments = arguments.into_iter();

        if arguments.len() == 0 {
            Ok(Some(format!(
                "{}/channels/{}/commands",
                env::var("BASE_URL")?,
                self.db
                    .get_or_create_channel(&execution_context.get_channel())?
                    .ok_or_else(|| CommandError::InvalidArgument(
                        "can't add commands outside of channels".to_string()
                    ))?
                    .id
            )))
        } else {
            match execution_context.get_permissions().await {
                Permissions::ChannelMod | Permissions::Admin => {
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

                            match self.db.add_command_to_channel(
                                &execution_context.get_channel(),
                                command_name,
                                &command_action,
                            ) {
                                Ok(()) => Ok(Some("Command successfully added".to_string())),
                                Err(DatabaseError::DieselError(
                                    diesel::result::Error::DatabaseError(
                                        diesel::result::DatabaseErrorKind::UniqueViolation,
                                        _,
                                    ),
                                )) => Ok(Some("Command already exists".to_string())),
                                Err(e) => Err(CommandError::DatabaseError(e)),
                            }
                        }
                        "del" | "delete" | "remove" => {
                            let command_name = arguments.next().ok_or_else(|| {
                                CommandError::MissingArgument("command name".to_string())
                            })?;

                            match self.db.delete_command_from_channel(
                                &execution_context.get_channel(),
                                command_name,
                            ) {
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
                                .get_command(&execution_context.get_channel(), command_name)?
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
    DatabaseError(DatabaseError),
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
        Self::InvalidArgument(format!("expected a number"))
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
