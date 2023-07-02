pub mod context;
mod db;
mod http;
pub mod storage;
mod utils;

use self::{context::HebiContext, storage::ModuleStorage};
use super::error::CommandError;
use crate::database::Database;
use hebi::prelude::*;
use reqwest::Client;
use std::time::Duration;
use tokio::time::timeout;
use tracing::instrument;

const TIMEOUT_SECS: u64 = 10;

#[instrument(skip(native_modules, module_storage))]
pub async fn eval_hebi(
    source: String,
    native_modules: &[NativeModule],
    module_storage: ModuleStorage,
    db: Database,
    args: &[String],
    ctx: HebiContext,
) -> Result<Option<String>, CommandError> {
    let mut hebi = Hebi::builder().module_loader(module_storage).finish();

    {
        let args_list = hebi.new_list(args.len());

        for arg in args {
            let arg_value = hebi.new_string(arg).into_value(hebi.global()).unwrap();
            args_list.push(arg_value);
        }
        let args_value = args_list.into_value(hebi.global()).unwrap();

        hebi.global().set(hebi.new_string("args"), args_value);
    }

    for module in native_modules {
        hebi.register(module);
    }

    let db_module = NativeModule::builder("db")
        .function("get", {
            let db = db.clone();
            let ctx = ctx.clone();
            move |scope| db::get(scope, db.clone(), ctx.clone())
        })
        .function("set", {
            let db = db.clone();
            let ctx = ctx.clone();
            move |scope| db::set(scope, db.clone(), ctx.clone())
        })
        .function("remove", {
            let db = db.clone();
            let ctx = ctx.clone();
            move |scope| db::remove(scope, db.clone(), ctx.clone())
        })
        .finish();

    hebi.register(&db_module);

    hebi.global()
        .set(hebi.new_string("context"), hebi.new_instance(ctx).unwrap());

    let eval_future = hebi.eval_async(&source);

    match timeout(Duration::from_secs(TIMEOUT_SECS), eval_future).await {
        Ok(Ok(value)) => Ok(Some(value.to_string())),
        Ok(Err(err)) => Err(CommandError::GenericError(err.to_string())),
        Err(_) => Err(CommandError::GenericError("Execution timed out".to_owned())),
    }
}

pub fn create_native_modules(_: Database) -> Vec<NativeModule> {
    let mut modules = Vec::new();

    let http_client = Client::new();

    let http = NativeModule::builder("http")
        .async_function("fetch", move |scope| {
            http::request(scope, http_client.clone())
        })
        .finish();
    modules.push(http);

    let utils = NativeModule::builder("utils")
        .function("format", utils::format_string)
        .function("to_int", utils::to_int)
        .async_function("sleep", utils::sleep)
        .finish();
    modules.push(utils);

    let context_module = NativeModule::builder("context")
        .class::<HebiContext>("Context", |class| {
            class
                .field("channel_id", |_, this| this.channel_id as i32)
                .finish()
        })
        .finish();

    modules.push(context_module);

    modules
}
