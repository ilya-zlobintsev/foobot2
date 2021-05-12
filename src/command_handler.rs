mod inquiry_helper;
pub mod twitch_api;

use core::fmt;
use std::{env::{self, VarError}, sync::Arc, time::Instant};

use crate::{
    database::{models::User, Database},
    platform::{ExecutionContext, Permissions, UserIdentifier},
};

use inquiry_helper::*;
use rocket_contrib::templates::handlebars::{Handlebars, TemplateRenderError};
use twitch_api::TwitchApi;

#[derive(Clone)]
pub struct CommandHandler {
    pub db: Database,
    pub twitch_api: Option<TwitchApi>,
    startup_time: Instant,
    template_registry: Arc<Handlebars<'static>>,
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

        let startup_time = Instant::now();

        let mut template_registry = Handlebars::new();

        template_registry.register_helper("context", Box::new(ContextHelper));

        Self {
            db,
            twitch_api,
            startup_time,
            template_registry: Arc::new(template_registry),
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
                Err(e) => Some(format!("Error: {}", e)),
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

        let user = self.db.get_user(user_identifier)?;

        match command {
            "ping" => Ok(Some(self.ping())),
            "whoami" | "id" => Ok(Some(format!(
                "{:?}, permissions: {:?}",
                user, execution_context.permissions
            ))),
            "cmd" | "command" | "commands" => self.cmd(command, arguments, execution_context).await,
            // Old commands for convenience
            "addcmd" | "cmdadd" => {
                self.cmd(
                    "command",
                    {
                        let mut arguments = arguments;
                        arguments.insert(0, "add");
                        arguments
                    },
                    execution_context,
                )
                .await
            }
            "delcmd" | "cmddel" => {
                self.cmd(
                    "command",
                    {
                        let mut arguments = arguments;
                        arguments.insert(0, "remove");
                        arguments
                    },
                    execution_context,
                )
                .await
            }
            "showcmd" | "checkcmd" => {
                self.cmd(
                    "command",
                    {
                        let mut arguments = arguments;
                        arguments.insert(0, "show");
                        arguments
                    },
                    execution_context,
                )
                .await
            }
            "debug" | "check" | "test" => {
                let action = arguments.join(" ");

                self.execute_command_action(&action, execution_context, user)
            }
            _ => match self.db.get_command(&execution_context.channel, command)? {
                Some(cmd) => self.execute_command_action(&cmd.action, execution_context, user),
                None => Ok(None),
            },
        }
    }

    fn execute_command_action(
        &self,
        action: &str,
        execution_context: ExecutionContext,
        user: User,
    ) -> Result<Option<String>, CommandError> {
        tracing::info!("Parsing action {}", action);

        let inquiry_context = InquiryContext {
            execution_context,
            user,
        };

        let response = self
            .template_registry
            .render_template(action, &inquiry_context)?;

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

    async fn cmd(
        &self,
        command: &str,
        arguments: Vec<&str>,
        execution_context: ExecutionContext,
    ) -> Result<Option<String>, CommandError> {
        let mut arguments = arguments.into_iter();

        if arguments.len() == 0 {
            Ok(Some(format!("{}/channels/{}/commands", env::var("BASE_URL")?, self.db.get_channel(execution_context.channel)?.id)))
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
                                execution_context.channel,
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
                                .delete_command(execution_context.channel, command_name)
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
    TemplateError(TemplateRenderError),
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
            CommandError::DatabaseError(e) => f.write_str(&format!("database error: {}", e)),
            CommandError::TemplateError(e) => f.write_str(&format!("inquiry error: {}", e)),
            CommandError::ConfigurationError(e) => f.write_str(&format!("configuration error: {}", e)),
        }
    }
}

impl From<diesel::result::Error> for CommandError {
    fn from(e: diesel::result::Error) -> Self {
        Self::DatabaseError(e)
    }
}

impl From<TemplateRenderError> for CommandError {
    fn from(e: TemplateRenderError) -> Self {
        Self::TemplateError(e)
    }
}

impl From<VarError> for CommandError {
    fn from(e: VarError) -> Self {
        Self::ConfigurationError(e)
    }
}

pub trait CommandMessage {
    fn get_user_identifier(&self) -> UserIdentifier;

    fn get_text(&self) -> String;
}
