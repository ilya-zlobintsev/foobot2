use super::*;

#[derive(Debug, Clone)]
pub struct WhoAmI;

#[async_trait]
impl ExecutableCommand for WhoAmI {
    fn get_names(&self) -> &[&str] {
        &["whoami", "id"]
    }

    fn get_cooldown(&self) -> u64 {
        5
    }

    fn get_permissions(&self) -> Permissions {
        Permissions::Default
    }

    async fn execute<C: ExecutionContext + Send + Sync>(
        &self,
        ctx: C,
        _trigger_name: &str,
        _args: Vec<&str>,
        (user, user_identifier): (&User, &UserIdentifier),
    ) -> Result<Option<String>, CommandError> {
        Ok(Some(format!(
            "{:?}, identified as {}, channel: {}, permissions: {:?}",
            user,
            user_identifier,
            ctx.get_channel(),
            ctx.get_permissions().await,
        )))
    }
}
