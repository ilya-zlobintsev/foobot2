mod http;
mod utils;

use super::error::CommandError;
use hebi::{Hebi, IntoValue, NativeModule};
use reqwest::Client;
use std::time::Duration;
use tokio::time::timeout;
use tracing::instrument;

const TIMEOUT_SECS: u64 = 10;

#[instrument(skip_all)]
pub async fn eval_hebi(
    source: String,
    native_modules: &[NativeModule],
    args: &[String],
) -> Result<Option<String>, CommandError> {
    let mut hebi = Hebi::new();

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

    let eval_future = hebi.eval_async(&source);

    match timeout(Duration::from_secs(TIMEOUT_SECS), eval_future).await {
        Ok(Ok(value)) => Ok(Some(value.to_string())),
        Ok(Err(err)) => Err(CommandError::GenericError(err.to_string())),
        Err(_) => Err(CommandError::GenericError("Execution timed out".to_owned())),
    }
}

pub fn create_native_modules() -> Vec<NativeModule> {
    let mut modules = Vec::new();

    let http_client = Client::new();

    let http = NativeModule::builder("http")
        .async_function("fetch", move |scope| {
            http::request(scope, http_client.clone())
        })
        .finish();
    modules.push(http);

    let utils = NativeModule::builder("utils")
        .function("len", utils::list_len)
        .function("push", utils::list_push)
        .function("join", utils::join)
        .function("format", utils::format_string)
        .finish();
    modules.push(utils);

    modules
}
