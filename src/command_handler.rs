pub mod twitch_api;

use core::fmt;
use std::{env, time::Instant};

use crate::{
    database::Database,
    platform::{ExecutionContext, Permissions, UserIdentifier},
};

use twitch_api::TwitchApi;

#[derive(Clone)]
pub struct CommandHandler {
    db: Database,
    pub twitch_api: Option<TwitchApi>,
    startup_time: Instant,
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

        Self {
            db,
            twitch_api,
            startup_time,
        }
    }

    /// This function expects a raw message that appears to be a command without the leading command prefix.
    pub async fn handle_command_message<T>(
        &self,
        message: &T,
        context: ExecutionContext,
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
                .run_command(&command, arguments, message.get_user_identifier(), context)
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
        user_identifier: UserIdentifier,
        execution_context: ExecutionContext,
    ) -> Result<Option<String>, CommandError> {
        tracing::info!("Processing command {} with {:?}", command, arguments);

        match command {
            "ping" => Ok(Some(self.ping())),
            "whoami" | "id" => Ok(Some(format!(
                "{:?}, permissions: {:?}",
                self.db.get_user(user_identifier),
                execution_context.permissions
            ))),
            "cmd" | "command" | "commands" => {
                let mut arguments = arguments.into_iter();

                if arguments.len() == 0 {
                    // TODO show command list
                    Ok(Some("Command list".to_string()))
                } else {
                    match execution_context.permissions {
                        Permissions::ChannelMod => {
                            match arguments.next().ok_or_else(|| {
                                CommandError::MissingArgument(
                                    "must be either add or delete".to_string(),
                                )
                            })? {
                                "add" | "create" => {
                                    let command_name = arguments.next().ok_or_else(|| {
                                        CommandError::MissingArgument("command name".to_string())
                                    })?;

                                    let command_action = arguments.collect::<Vec<&str>>().join(" ");
                                    
                                    if command_action.is_empty() {
                                        return Err(CommandError::MissingArgument("command action".to_string()))
                                    }

                                    match self.db.add_command(
                                        execution_context.channel,
                                        command_name,
                                        &command_action,
                                    ) {
                                        Ok(()) => Ok(Some(format!("Command successfully added"))),
                                        Err(diesel::result::Error::DatabaseError(
                                            diesel::result::DatabaseErrorKind::UniqueViolation,
                                            _,
                                        )) => Ok(Some("Command already exists".to_string())),
                                        Err(e) => Err(CommandError::DatabaseError(e)),
                                    }
                                }
                                "del" | "delete" | "remove" => {
                                    Ok(Some("Unimplemented".to_string()))
                                }
                                _ => Err(CommandError::InvalidArgument(command.to_string())),
                            }
                        }
                        Permissions::Default => Err(CommandError::NoPermissions),
                    }
                }
            }
            _ => match self.db.get_command(execution_context.channel, command)? {
                Some(cmd) => Ok(Some(cmd.action)),
                None => Ok(None),
            },
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
}

#[derive(Debug)]
pub enum CommandError {
    MissingArgument(String),
    InvalidArgument(String),
    NoPermissions,
    DatabaseError(diesel::result::Error),
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
        }
    }
}

impl From<diesel::result::Error> for CommandError {
    fn from(e: diesel::result::Error) -> Self {
        Self::DatabaseError(e)
    }
}

pub trait CommandMessage {
    fn get_user_identifier(&self) -> UserIdentifier;

    fn get_text(&self) -> String;
}
