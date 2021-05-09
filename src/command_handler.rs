pub mod twitch_api;

use core::fmt;
use std::{env, time::Instant};

use crate::{database::Database, platform::{ExecutionContext, UserIdentifier}};

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

    pub async fn handle_command_message<T>(&self, message: &T, context: ExecutionContext) -> Option<String> where T: Sync + CommandMessage {
        let message_text = message.get_text();

        let mut split = message_text.split_whitespace();
        
        let command = split.next().unwrap().to_owned();
        
        let arguments: Vec<&str> = split.collect();
        
        match self.run_command(&command, arguments, context).await {
            Ok(result) => result,
            Err(e) => Some(format!("Error: {}", e))
        }
    }

    // #[async_recursion]
    async fn run_command(
        &self,
        command: &str,
        arguments: Vec<&str>,
        execution_context: ExecutionContext,
    ) -> Result<Option<String>, CommandError> {
        tracing::info!("Processing command {} with {:?}", command, arguments);

        Ok(match command {
            "ping" => Some(self.ping()),
            "cmd" => {
                let mut arguments = arguments.into_iter();

                match arguments.next().ok_or_else(|| {
                    CommandError::MissingArgument("must be either add or delete".to_string())
                })? {
                    "add" | "create" => {
                        let command_name = arguments.next().ok_or_else(|| {
                            CommandError::MissingArgument("command name".to_string())
                        })?;
                        let command_action = arguments.collect::<Vec<&str>>().join(" ");

                        Some("Adding command".to_string())
                    }
                    "del" | "delete" | "remove" => Some("unimplemented".to_string()),
                    _ => return Err(CommandError::InvalidArgument(command.to_string())),
                }
                // self.run_command(command, iter.collect(), execution_context).await?
            }
            _ => None,
        })
    }

    fn ping(&self) -> String {
        let uptime = {
            let duration = self.startup_time.elapsed();

            let minutes = (duration.as_secs() / 60) % 60;
            let hours = (duration.as_secs() / 60) / 60;

            let mut result = String::new();

            if hours != 0 {
                result.push_str(&format!("{} hours ", hours));
            };

            if minutes != 0 {
                result.push_str(&format!("{} minutes ", minutes));
            }

            if result.is_empty() {
                result.push_str(&format!("{} seconds", duration.as_secs()));
            }

            result
        };

        format!("Pong! Uptime {}", uptime)
    }
}

pub enum CommandError {
    MissingArgument(String),
    InvalidArgument(String),
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
        }
    }
}

pub trait CommandMessage {
    fn get_user_identifier(&self) -> UserIdentifier;
    
    fn get_text(&self) -> String;
}
