use super::*;
use crate::command_handler::execute_command_action;
use handlebars::Handlebars;
use std::sync::Arc;

pub struct Debug {
    template_registry: Arc<Handlebars<'static>>,
}

#[async_trait]
impl ExecutableCommand for Debug {
    fn get_names(&self) -> &[&str] {
        &["debug"]
    }

    fn get_cooldown(&self) -> u64 {
        0
    }

    fn get_permissions(&self) -> Permissions {
        Permissions::ChannelMod
    }

    async fn execute<'a, P: PlatformContext + Send + Sync>(
        &self,
        ctx: ExecutionContext<'a, P>,
        _trigger_name: &str,
        args: Vec<&str>,
    ) -> Result<Option<String>, CommandError> {
        let action = args.join(" ");
        execute_command_action(self.template_registry.clone(), action, &ctx, vec![]).await
    }
}

impl Debug {
    pub fn new(template_registry: Arc<Handlebars<'static>>) -> Self {
        Self { template_registry }
    }
}
