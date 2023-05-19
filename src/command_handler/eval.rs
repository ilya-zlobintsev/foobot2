use super::error::CommandError;
use hebi::{Hebi, NativeModule, Scope, Str};
use reqwest::Client;
use tokio_util::task::LocalPoolHandle;

pub async fn eval_hebi(
    source: String,
    pool: &LocalPoolHandle,
    // native_modules: &[NativeModule],
) -> Result<Option<String>, CommandError> {
    pool.spawn_pinned(|| async move {
        let mut hebi = Hebi::new();

        let native_modules = create_native_modules();
        for module in native_modules {
            hebi.register(&module);
        }

        match hebi.eval_async(&source).await {
            Ok(value) => Ok(Some(value.to_string())),
            Err(err) => Err(CommandError::GenericError(err.to_string())),
        }
    })
    .await
    .unwrap()
}

pub fn create_native_modules() -> Vec<NativeModule> {
    let mut modules = Vec::new();

    let http_client = Client::new();

    let http_module = NativeModule::builder("http")
        .async_function("get", move |scope| get(scope, http_client.clone()))
        .finish();
    modules.push(http_module);

    /*let json_module = NativeModule::builder("json")
        .class::<Value>("Value", |class| {
            class
                .init(|_| Ok(Value::Null))
                .method("get", |scope, this| {
                    let index = scope.param::<Str>(0)?;
                    let inner_value = this.get(index.as_str()).ok_or_else(|| {
                        hebi::Error::User(format!("Unrecognized field: `{index}`").into())
                    })?;
                    Ok(scope.new_instance(inner_value.clone()))
                })
                .finish()
        })
        .finish();
    modules.push(json_module);*/

    modules
}

async fn get(scope: Scope<'_>, client: Client) -> hebi::Result<Str<'_>> {
    let url = scope.param::<Str>(0)?;

    match scope.param::<Str>(1).map(|str| str.to_string()).as_deref() {
        Ok("plain") | Err(_) => {
            let response = client
                .get(url.as_str())
                .send()
                .await
                .map_err(|err| hebi::Error::User(Box::new(err)))?;
            let text = response
                .text()
                .await
                .map_err(|err| hebi::Error::User(Box::new(err)))?;
            Ok(scope.new_string(text))
        }
        Ok(other) => Err(hebi::Error::User(
            format!("Unsupported format `{other}`").into(),
        )),
    }
}
