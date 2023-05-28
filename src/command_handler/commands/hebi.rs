use super::*;
use crate::command_handler::eval::{eval_hebi, storage::ModuleStorage};
use ::hebi::NativeModule;

pub struct DebugHebi {
    native_modules: Arc<Vec<NativeModule>>,
    module_storage: ModuleStorage,
}

#[async_trait]
impl ExecutableCommand for DebugHebi {
    fn get_names(&self) -> &[&str] {
        &["debug_hebi"]
    }

    fn get_cooldown(&self) -> u64 {
        0
    }

    fn get_permissions(&self) -> Permissions {
        Permissions::ChannelMod
    }

    async fn execute<'a, P: PlatformContext + Send + Sync>(
        &self,
        _: &ExecutionContext<'a, P>,
        _trigger_name: &str,
        args: Vec<&str>,
    ) -> Result<Option<String>, CommandError> {
        let action = args.join(" ");

        eval_hebi(
            action,
            &self.native_modules,
            self.module_storage.clone(),
            &[],
        )
        .await
    }
}

impl DebugHebi {
    pub fn new(native_modules: Arc<Vec<NativeModule>>, module_storage: ModuleStorage) -> Self {
        Self {
            native_modules,
            module_storage,
        }
    }
}
