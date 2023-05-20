mod http;

use super::error::CommandError;
use hebi::{Hebi, NativeModule};
use reqwest::Client;
use std::time::Duration;
use tokio::time::timeout;

const TIMEOUT_SECS: u64 = 10;

pub async fn eval_hebi(
    source: String,
    native_modules: &[NativeModule],
) -> Result<Option<String>, CommandError> {
    let mut hebi = Hebi::new();

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

    let http_module = NativeModule::builder("http")
        .async_function("get", move |scope| http::get(scope, http_client.clone()))
        .finish();
    modules.push(http_module);

    modules
}
