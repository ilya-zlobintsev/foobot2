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
        cmd: BuiltinCommand,
        _: &CommandHandler,
        _: Vec<&str>,
        execution_context: &C,
        user: &User,
    ) -> Result<String, CommandError> {
        todo!()
    }
}
