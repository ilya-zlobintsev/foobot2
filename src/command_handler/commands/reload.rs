use std::str::FromStr;

use super::*;
use crate::command_handler::eval::storage::ModuleStorage;
use strum::EnumString;

#[derive(Debug, Clone)]
pub struct Reload {
    pub module_storage: ModuleStorage,
}

#[derive(EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum Subcommand {
    Hebi,
}

#[async_trait]
impl ExecutableCommand for Reload {
    fn get_names(&self) -> &[&str] {
        &["reload"]
    }

    fn get_cooldown(&self) -> u64 {
        0
    }

    fn get_permissions(&self) -> Permissions {
        Permissions::Admin
    }

    async fn execute<'a, P: PlatformContext + Send + Sync>(
        &self,
        _: &ExecutionContext<'a, P>,
        _: &str,
        args: Vec<&str>,
    ) -> Result<Option<String>, CommandError> {
        let raw_subcommand = args
            .first()
            .ok_or_else(|| CommandError::MissingArgument("subcommand".to_owned()))?;
        let subcommand = Subcommand::from_str(raw_subcommand).map_err(|_| {
            CommandError::InvalidArgument(format!("Invalid subcommand: {raw_subcommand}"))
        })?;

        match subcommand {
            Subcommand::Hebi => match self.module_storage.update() {
                Ok(Some(commit)) => Ok(Some(format!(
                    "Hebi modules were updated to commit {commit}"
                ))),
                Ok(None) => Ok(Some("Hebi modules are already up to date".to_owned())),
                Err(err) => Err(CommandError::GenericError(format!(
                    "Could not reload hebi modules: {err:#}"
                ))),
            },
        }
    }
}
