pub mod builtin;

use tracing::error;
use super::{CommandError, CommandHandler};
use crate::database::models::Command as CustomCommand;
use crate::{
    database::models::User,
    platform::{ExecutionContext, Permissions},
};
use std::str::FromStr;

type CommandResult = std::result::Result<String, CommandError>;

#[async_trait]
pub trait Command: std::fmt::Display {
    async fn execute<C: ExecutionContext + Sync>(
        self,
        handler: &CommandHandler,
        args: Vec<&str>,
        execution_context: &C,
        user: &User,
    ) -> CommandResult;

    fn get_cooldown(&self) -> u64;

    fn get_permissions(&self) -> Permissions;
}

#[async_trait]
impl Command for CustomCommand {
    async fn execute<C: ExecutionContext + Sync>(
        self,
        handler: &CommandHandler,
        args: Vec<&str>,
        execution_context: &C,
        user: &User,
    ) -> CommandResult {
        handler
            .execute_command_action(
                self.action,
                execution_context,
                user.clone(),
                args.into_iter().map(|s| s.to_owned()).collect(),
            )
            .await
    }

    fn get_cooldown(&self) -> u64 {
        self.cooldown.unwrap_or_default()
    }

    fn get_permissions(&self) -> Permissions {
        self.permissions
            .as_ref()
            .and_then(|s| {
                Permissions::from_str(&s)
                    .map_err(|error| {
                        error!("Failed to parse permissions from a custom command: {error}")
                    })
                    .ok()
            })
            .unwrap_or_default()
    }
}
