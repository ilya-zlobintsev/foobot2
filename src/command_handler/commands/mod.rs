mod cmd;
mod debug;
mod ping;
mod shell;
mod twitch_eventsub;
mod whoami;

use self::{
    cmd::Cmd, debug::Debug, ping::Ping, shell::Shell, twitch_eventsub::TwitchEventSub,
    whoami::WhoAmI,
};
use super::{CommandError, ExecutionContext};
use crate::platform::{Permissions, PlatformContext};
use async_trait::async_trait;
use enum_dispatch::enum_dispatch;
use handlebars::Handlebars;
use std::sync::Arc;

#[async_trait]
#[enum_dispatch]
pub trait ExecutableCommand {
    fn get_names(&self) -> &[&str];

    fn get_cooldown(&self) -> u64;

    fn get_permissions(&self) -> Permissions;

    async fn execute<'a, P: PlatformContext + Send + Sync>(
        &self,
        ctx: &ExecutionContext<'a, P>,
        trigger_name: &str,
        args: Vec<&str>,
    ) -> Result<Option<String>, CommandError>;
}

#[enum_dispatch(ExecutableCommand)]
#[derive(strum::Display)]
pub enum BuiltinCommand {
    Ping(Ping),
    Debug(Debug),
    Cmd(Cmd),
    WhoAmI(WhoAmI),
    Shell(Shell),
    TwitchEventSub(TwitchEventSub),
}

impl std::fmt::Debug for BuiltinCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

pub fn create_builtin_commands(template_registry: Arc<Handlebars<'static>>) -> Vec<BuiltinCommand> {
    vec![
        Ping::default().into(),
        Debug::new(template_registry).into(),
        Cmd.into(),
        WhoAmI.into(),
        Shell.into(),
        TwitchEventSub.into(),
    ]
}
