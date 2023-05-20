use hebi::{IntoValue, Scope, Str};
use reqwest::Client;
use serde::de::DeserializeSeed;

pub async fn get(scope: Scope<'_>, client: Client) -> hebi::Result<hebi::Value<'_>> {
    let url = scope.param::<Str>(0)?;

    let response = client
        .get(url.as_str())
        .send()
        .await
        .map_err(|err| hebi::Error::User(Box::new(err)))?;
    let text = response
        .text()
        .await
        .map_err(|err| hebi::Error::User(Box::new(err)))?;

    match scope.param::<Str>(1).map(|str| str.to_string()).as_deref() {
        Ok("plain") | Err(_) => scope.new_string(text).into_value(scope.global()),
        Ok("json") => {
            let mut json_deserializer = serde_json::Deserializer::from_str(&text);
            let hebi_deserializer = hebi::ValueDeserializer::new(scope.global());
            let value = hebi_deserializer
                .deserialize(&mut json_deserializer)
                .map_err(hebi::Error::user)?;
            Ok(value)
        }
        Ok(other) => Err(hebi::Error::User(
            format!("Unsupported format `{other}`").into(),
        )),
    }
}
