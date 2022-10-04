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

    async fn execute<'a, P: PlatformContext + Send + Sync>(
        &self,
        ctx: &ExecutionContext<'a, P>,
        _trigger_name: &str,
        _args: Vec<&str>,
    ) -> Result<Option<String>, CommandError> {
        Ok(Some(format!(
            "{:?}, identified as {}, channel: {}, permissions: {:?}",
            ctx.user,
            ctx.platform_ctx.get_user_identifier(),
            ctx.platform_ctx.get_channel(),
            ctx.platform_ctx.get_permissions().await,
        )))
    }
}
