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
use super::{platform_handler::PlatformHandler, CommandError};
use crate::{
    database::{models::User, Database},
    platform::{ExecutionContext, Permissions, UserIdentifier},
};
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

    async fn execute<C: ExecutionContext + Send + Sync>(
        &self,
        ctx: C,
        trigger_name: &str,
        args: Vec<&str>,
        (user, user_identifier): (&User, &UserIdentifier),
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

pub fn create_builtin_commands(
    db: Database,
    template_registry: Arc<Handlebars<'static>>,
    platform_handler: &PlatformHandler,
) -> Vec<BuiltinCommand> {
    let mut commands = Vec::new();

    commands.push(Ping::default().into());
    commands.push(Debug::new(db.clone(), template_registry).into());
    commands.push(Cmd::new(db.clone()).into());
    commands.push(WhoAmI.into());
    commands.push(Shell.into());

    if let Some(twitch_api) = &platform_handler.twitch_api {
        commands.push(TwitchEventSub::new(db, twitch_api.helix_api_app.clone()).into());
    }

    commands
}
