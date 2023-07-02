use hebi::{prelude::*, Result};
use std::time::Duration;

pub fn format_string(scope: Scope<'_>) -> Result<Str<'_>> {
    let mut input = scope.param::<Str>(0)?.to_string();

    let mut i = 1;
    while let Ok(arg) = scope.param::<Str>(i) {
        input = input.replacen("{}", arg.as_str(), 1);
        i += 1;
    }

    Ok(scope.new_string(input))
}

pub fn to_int(scope: Scope<'_>) -> Result<i32> {
    let input = scope.param::<Value>(0)?;
    input.to_string().parse().map_err(hebi::Error::user)
}

pub async fn sleep(scope: Scope<'_>) -> Result<()> {
    let ms = scope.param::<i32>(0)?;
    tokio::time::sleep(Duration::from_millis(ms as u64)).await;
    Ok(())
}
