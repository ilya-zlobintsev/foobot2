use tokio::process::Command;
use tracing::warn;

use super::*;
use std::env;

#[derive(Debug, Clone)]
pub struct Shell;

#[async_trait]
impl ExecutableCommand for Shell {
    fn get_names(&self) -> &[&str] {
        &["shell", "sh"]
    }

    fn get_cooldown(&self) -> u64 {
        0
    }

    fn get_permissions(&self) -> Permissions {
        Permissions::Admin
    }

    async fn execute<'a, P: PlatformContext + Send + Sync>(
        &self,
        _ctx: ExecutionContext<'a, P>,
        _trigger_name: &str,
        args: Vec<&str>,
    ) -> Result<Option<String>, CommandError> {
        if let Ok("1") = env::var("ALLOW_SHELL").as_deref() {
            match Command::new("sh").arg("-c").args(args).output().await {
                Ok(output) => {
                    let stdout = String::from_utf8(output.stdout)
                        .unwrap_or_else(|_| "<invalid UTF-8>".to_owned());
                    let stderr = String::from_utf8(output.stderr)
                        .unwrap_or_else(|_| "<invalid UTF-8>".to_owned());
                    let final_output = format!("{stdout}\n{stderr}").trim().to_owned();

                    Ok(Some(final_output))
                }
                Err(err) => Err(CommandError::GenericError(format!(
                    "could not run command: {err}"
                ))),
            }
        } else {
            warn!("Trying to use `shell` when ALLOW_SHELL isn't set to 1");
            Err(CommandError::NoPermissions)
        }
    }
}
