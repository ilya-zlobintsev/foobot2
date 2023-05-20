use hebi::{List, Result, Scope, Str, Value};

pub fn get_list_element(scope: Scope<'_>) -> Result<Option<Value<'_>>> {
    let list = scope.param::<List>(0)?;
    let index = scope.param::<i32>(1)?;
    Ok(list.get(index as usize))
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
