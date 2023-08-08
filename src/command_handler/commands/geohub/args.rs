use crate::command_handler::error::CommandError;
use clap::{Parser, Subcommand};
use std::{ffi::OsString, iter, str::FromStr};

#[derive(Clone, Debug, Parser)]
pub struct CommandArgs {
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Clone, Debug, Subcommand, PartialEq)]
pub enum Command {
    #[command(subcommand)]
    Leaderboard(LeaderboardCommand),
}

// #[derive(Parser)]
#[derive(Clone, Debug, Subcommand, PartialEq)]
pub enum LeaderboardCommand {
    Daily {
        #[arg(long, default_value_t)]
        channel: bool,
    },
}

impl CommandArgs {
    pub fn parse_from_args(args: &[&str]) -> Result<Self, CommandError> {
        let args = iter::once(&"geohub")
            .chain(args)
            .map(|item| {
                OsString::from_str(item)
                    .map_err(|err| CommandError::InvalidArgument(err.to_string()))
            })
            .collect::<Result<Vec<OsString>, CommandError>>()?;

        Self::try_parse_from(args).map_err(|err| CommandError::InvalidArgument(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic() {
        let input = "leaderboard daily --channel"
            .split_whitespace()
            .collect::<Vec<_>>();
        let args = CommandArgs::parse_from_args(&input).unwrap();
        assert_eq!(
            args.cmd,
            Command::Leaderboard(LeaderboardCommand::Daily { channel: true })
        );
    }
}
