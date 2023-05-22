use hebi::{List, Result, Scope, Str, Value};

pub fn list_len(scope: Scope<'_>) -> Result<i32> {
    let list = scope.param::<List>(0)?;
    Ok(list.len() as i32)
}

pub fn list_push(scope: Scope<'_>) -> Result<()> {
    let list = scope.param::<List>(0)?;
    let value = scope.param::<Value>(1)?;
    list.push(value);
    Ok(())
}

pub fn join(scope: Scope<'_>) -> Result<Str> {
    let list = scope.param::<List>(0)?;
    let separator = scope.param::<Str>(1)?;

    let output = list
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(separator.as_str());

    Ok(scope.new_string(output))
}

pub fn format_string(scope: Scope<'_>) -> Result<Str<'_>> {
    let mut input = scope.param::<Str>(0)?.to_string();

    let mut i = 1;
    while let Ok(arg) = scope.param::<Str>(i) {
        input = input.replacen("{}", arg.as_str(), 1);
        i += 1;
    }

    Ok(scope.new_string(input))
}
