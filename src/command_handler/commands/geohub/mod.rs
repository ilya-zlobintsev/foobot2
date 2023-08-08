mod args;
mod client;

use self::{
    args::{Command, LeaderboardCommand},
    client::GeohubClient,
};

use super::ExecutableCommand;
use crate::{
    command_handler::{commands::geohub::args::CommandArgs, error::CommandError, ExecutionContext},
    platform::PlatformContext,
};
use async_trait::async_trait;

#[derive(Default)]
pub struct GeoHub {
    client: GeohubClient,
}

#[async_trait]
impl ExecutableCommand for GeoHub {
    fn get_names(&self) -> &[&str] {
        &["geohub"]
    }

    fn get_cooldown(&self) -> u64 {
        5
    }

    async fn execute<'a, P: PlatformContext + Send + Sync>(
        &self,
        _ctx: &ExecutionContext<'a, P>,
        _trigger_name: &str,
        args: Vec<&str>,
    ) -> Result<Option<String>, CommandError> {
        let input = CommandArgs::parse_from_args(&args)?;

        match input.cmd {
            Command::Leaderboard(LeaderboardCommand::Daily { .. }) => {
                let leaderboard = self.client.get_leaderboard(10).await?;
                let users_output = leaderboard
                    .today
                    .into_iter()
                    .map(|entry| format!("{}: {}", entry.user_name, entry.total_points))
                    .collect::<Vec<String>>()
                    .join(", ");
                Ok(Some(format!("Top daily scores: {users_output}")))
            }
        }
    }
}
