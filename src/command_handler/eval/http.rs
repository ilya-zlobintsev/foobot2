use hebi::{public::Table, IntoValue, Scope, Str};
use http::Method;
use reqwest::Client;
use serde::de::DeserializeSeed;
use std::str::FromStr;
use tracing::debug;

pub async fn request(scope: Scope<'_>, client: Client) -> hebi::Result<hebi::Value<'_>> {
    let url = scope.param::<Str>(0)?;
    let request_params = scope
        .param::<Table>(1)
        .unwrap_or_else(|_| scope.new_table(0));

    let raw_method = get_str_param(&request_params, &scope, "method", "GET");
    let format = get_str_param(&request_params, &scope, "format", "plain");

    let method =
        Method::from_str(raw_method.as_str()).map_err(|err| hebi::Error::User(Box::new(err)))?;

    debug!("Sending {method} request to {url}");

    let response = client
        .request(method, url.as_str())
        .send()
        .await
        .map_err(|err| hebi::Error::User(Box::new(err)))?;

    let text = response
        .text()
        .await
        .map_err(|err| hebi::Error::User(Box::new(err)))?;

    match format.as_str() {
        "plain" | "text" => scope.new_string(text).into_value(scope.global()),
        "json" => {
            let mut json_deserializer = serde_json::Deserializer::from_str(&text);
            let hebi_deserializer = hebi::ValueDeserializer::new(scope.global());
            let value = hebi_deserializer
                .deserialize(&mut json_deserializer)
                .map_err(|err| hebi::Error::User(format!("Deserialization error: {err}").into()))?;
            Ok(value)
        }
        other => Err(hebi::Error::User(
            format!("Unsupported format `{other}`").into(),
        )),
    }
}

fn get_str_param<'a>(table: &Table<'a>, scope: &Scope<'a>, key: &str, default: &str) -> Str<'a> {
    table
        .get(key)
        .and_then(|obj| obj.as_object::<Str>(scope.global()))
        .unwrap_or_else(|| scope.new_string(default))
}
