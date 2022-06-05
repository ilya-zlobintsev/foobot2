pub mod custom;
mod ping;

use ping::Command as Ping;

use super::CommandError;
use crate::platform::{ExecutionContext, Permissions};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

type Result<T> = std::result::Result<T, CommandError>;
pub type DynCommand = Arc<dyn ExecutableCommand + Send + Sync>;
pub type DynExecutionContext = Box<dyn ExecutionContext + Send + Sync>;

#[async_trait]
pub trait ExecutableCommand {
    async fn execute(
        &self,
        context: Box<dyn ExecutionContext + Send + Sync>,
        arguments: Vec<String>,
    ) -> Result<Option<String>>;

    fn get_cooldown(&self) -> u64;

    fn get_permissions(&self) -> Permissions;
}

pub fn create_builtin_commands() -> HashMap<String, DynCommand> {
    let mut commands = HashMap::new();

    commands.insert(
        "ping".to_owned(),
        as_dyn_command(Ping {
            startup_time: Instant::now(),
        }),
    );

    commands
}

pub fn as_dyn_command<T: ExecutableCommand + Send + Sync + 'static>(command: T) -> DynCommand {
    Arc::new(command) as DynCommand
}
