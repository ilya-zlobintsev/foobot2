pub mod builtin;

use super::{CommandError, CommandHandler};
use crate::database::models::Command as CustomCommand;
use crate::{
    database::models::User,
    platform::{ExecutionContext, Permissions},
};

type CommandResult = std::result::Result<String, CommandError>;

#[async_trait]
trait ExecutableCommand {
    async fn execute<C: ExecutionContext + Sync>(
        handler: &CommandHandler,
        args: Vec<&str>,
        execution_context: &C,
        user: &User,
    ) -> CommandResult;
}
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
        Permissions::Default // TODO
                             // self.permissions.unwrap_or_default()
    }
}
