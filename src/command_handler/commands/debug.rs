use super::*;
use crate::{command_handler::execute_command_action, database::Database};
use handlebars::Handlebars;
use std::sync::Arc;

pub struct Debug {
    template_registry: Arc<Handlebars<'static>>,
    db: Database,
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

    async fn execute<C: ExecutionContext + Send + Sync>(
        &self,
        ctx: C,
        _trigger_name: &str,
        args: Vec<&str>,
        _: (&User, &UserIdentifier),
    ) -> Result<Option<String>, CommandError> {
        let user = self.db.get_or_create_user(&ctx.get_user_identifier())?;
        let action = args.join(" ");

        execute_command_action(self.template_registry.clone(), action, ctx, user, vec![]).await
    }
}

impl Debug {
    pub fn new(db: Database, template_registry: Arc<Handlebars<'static>>) -> Self {
        Self {
            db,
            template_registry,
        }
    }
}
