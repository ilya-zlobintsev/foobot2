use crate::{
    command_handler::{CommandError, CommandHandler, commands::ExecutableCommand},
    database::models::User,
    platform::ExecutionContext,
};

pub struct Command;

#[async_trait]
impl ExecutableCommand for Command {
    async fn execute<C: ExecutionContext + Sync>(
        _: &CommandHandler,
        _: Vec<&str>,
        execution_context: &C,
        user: &User,
    ) -> Result<String, CommandError> {
        let user_identifier = execution_context.get_user_identifier();
        Ok(format!(
            "{:?}, identified as {}, channel: {}, permissions: {:?}",
            user,
            user_identifier,
            execution_context.get_channel(),
            execution_context.get_permissions().await,
        ))
    }
}
