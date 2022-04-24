mod cmd;
mod debug;
mod ping;
mod userinfo;

use cmd::Command as Cmd;
use debug::Command as Debug;
use ping::Command as Ping;
use userinfo::Command as UserInfo;

use super::{Command, CommandResult};
use crate::{
    command_handler::CommandHandler,
    database::models::User,
    platform::{ExecutionContext, Permissions},
};
use std::str::FromStr;
use strum::{AsRefStr, EnumIter, IntoEnumIterator};

#[async_trait]
trait ExecutableCommand {
    async fn execute<C: ExecutionContext + Sync>(
        cmd: BuiltinCommand,
        handler: &CommandHandler,
        args: Vec<&str>,
        execution_context: &C,
        user: &User,
    ) -> CommandResult;
}

#[derive(EnumIter, strum::Display, AsRefStr, PartialEq, Debug)]
#[strum(serialize_all = "lowercase")]
pub enum BuiltinCommand {
    Ping,
    UserInfo,
    Debug,
    Cmd,
    AddCmd,
    DelCmd,
    ShowCmd,
}

impl FromStr for BuiltinCommand {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::iter()
            .find(|variant| variant.get_names().contains(&s))
            .ok_or(())
    }
}

#[async_trait]
impl Command for BuiltinCommand {
    async fn execute<C: ExecutionContext + Sync>(
        self,
        handler: &CommandHandler,
        args: Vec<&str>,
        execution_context: &C,
        user: &User,
    ) -> CommandResult {
        let f = match self {
            Self::Ping => Ping::execute,
            Self::UserInfo => UserInfo::execute,
            Self::Debug => Debug::execute,
            Self::Cmd | Self::AddCmd | Self::DelCmd | Self::ShowCmd => Cmd::execute,
        };
        f(self, handler, args, execution_context, user).await
    }

    fn get_cooldown(&self) -> u64 {
        match self {
            Self::Ping => 10,
            Self::UserInfo => 15,
            Self::Debug => 0,
            Self::Cmd | Self::AddCmd | Self::DelCmd | Self::ShowCmd => 0,
        }
    }

    fn get_permissions(&self) -> Permissions {
        match self {
            Self::Ping => Permissions::Default,
            Self::UserInfo => Permissions::Default,
            Self::Debug => Permissions::ChannelMod,
            Self::Cmd | Self::AddCmd | Self::DelCmd | Self::ShowCmd => Permissions::ChannelMod,
        }
    }
}

impl BuiltinCommand {
    fn get_names(&self) -> Vec<&str> {
        let mut names = vec![self.as_ref()];
        names.extend(self.get_aliases());
        names
    }

    fn get_aliases(&self) -> Vec<&str> {
        match self {
            Self::Ping => vec!["пінг"],
            Self::UserInfo => vec!["whoami"],
            Self::Debug => vec![],
            Self::Cmd | Self::AddCmd | Self::DelCmd | Self::ShowCmd => vec![],
        }
    }
}
