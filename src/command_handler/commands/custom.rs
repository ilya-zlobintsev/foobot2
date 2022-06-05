use super::{DynExecutionContext, ExecutableCommand, Result};
use crate::command_handler::inquiry_helper::InquiryContext;
use crate::database::models::{Command as DbCommand, User};
use crate::platform::{ExecutionContext, Permissions};
use handlebars::Handlebars;
use std::sync::Arc;
use tokio::task;

pub struct CustomCommand {
    pub template_registry: Arc<Handlebars<'static>>,
    pub command: DbCommand,
    pub user: User,
}

#[async_trait]
impl ExecutableCommand for CustomCommand {
    async fn execute(
        &self,
        context: Box<dyn ExecutionContext + Send + Sync>,
        arguments: Vec<String>,
    ) -> Result<Option<String>> {
        tracing::debug!("Parsing action {}", self.command.action);

        execute_action(
            Arc::clone(&self.template_registry),
            self.command.action.clone(),
            arguments,
            self.user.clone(),
            context,
        )
        .await
    }

    fn get_cooldown(&self) -> u64 {
        5
    }

    fn get_permissions(&self) -> Permissions {
        todo!()
    }
}

pub async fn execute_action(
    template_registry: Arc<Handlebars<'static>>,
    action: String,
    arguments: Vec<String>,
    user: User,
    context: DynExecutionContext,
) -> Result<Option<String>> {
    let display_name = context.get_display_name().to_string();
    let channel = context.get_channel();

    let response = match task::spawn_blocking(move || {
        template_registry.render_template(
            &action,
            &(InquiryContext {
                user,
                arguments: arguments.iter().map(|s| s.to_owned()).collect(),
                display_name,
                channel,
            }),
        )
    })
    .await
    .expect("Failed to join")
    {
        Ok(result) => result,
        Err(e) => {
            tracing::debug!("Failed to render command template: {:?}", e);
            e.desc
        }
    };

    if response.is_empty() {
        Ok(None)
    } else {
        Ok(Some(response))
    }
}
