use super::{BuiltinCommand, ExecutableCommand};
use crate::{
    command_handler::{CommandError, CommandHandler},
    database::models::User,
    platform::ExecutionContext,
};

pub struct Command;

#[async_trait]
impl ExecutableCommand for Command {
    async fn execute<C: ExecutionContext + Sync>(
        _: BuiltinCommand,
        handler: &CommandHandler,
        args: Vec<&str>,
        execution_context: &C,
        user: &User,
    ) -> Result<String, CommandError> {
        let action = args.join(" ");
        handler
            .execute_command_action(
                action,
                execution_context,
                user.clone(),
                args.into_iter().map(|a| a.to_owned()).collect(),
            )
            .await
    }
}
