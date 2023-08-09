mod args;

use self::args::{Command, LeaderboardCommand};
use super::ExecutableCommand;
use crate::{
    command_handler::{
        commands::geohub::args::CommandArgs, error::CommandError, geohub::GeohubClient,
        ExecutionContext,
    },
    database::models::GeohubLink,
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
        ctx: &ExecutionContext<'a, P>,
        _trigger_name: &str,
        args: Vec<&str>,
    ) -> Result<Option<String>, CommandError> {
        let input = CommandArgs::parse_from_args(&args)?;
        let channel_id = ctx
            .channel_id
            .ok_or_else(|| CommandError::InvalidArgument("not running in a channel".to_owned()))?;

        match input.cmd {
            Command::Leaderboard(LeaderboardCommand::Daily { channel }) => {
                let limit = if channel { 200 } else { 10 };
                let leaderboard = self.client.get_leaderboard(limit).await?;

                let scores = if channel {
                    let names = ctx.db.get_geohub_link_names(channel_id)?;
                    leaderboard
                        .today
                        .into_iter()
                        .filter(|entry| names.contains(&entry.user_name))
                        .collect::<Vec<_>>()
                } else {
                    leaderboard.today
                };

                if scores.is_empty() {
                    return Ok(Some(
                        "Nobody has played the daily challenge yet!".to_owned(),
                    ));
                }

                let users_output = scores
                    .into_iter()
                    .map(|entry| format!("{}: {}", entry.user_name, entry.total_points))
                    .collect::<Vec<String>>()
                    .join(", ");
                Ok(Some(format!("Top daily challenge scores: {users_output}")))
            }
            Command::Link { username } => {
                let link = GeohubLink {
                    user_id: ctx.user.id,
                    channel_id,
                    geohub_name: username,
                };
                ctx.db.create_geohub_link(link)?;
                Ok(Some("Succesfully linked.".to_owned()))
            }
        }
    }
}
