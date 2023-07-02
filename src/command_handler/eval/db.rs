use super::context::HebiContext;
use crate::database::Database;
use hebi::prelude::*;
use tracing::{error, instrument};

#[instrument(name = "hebi.db.get", skip_all)]
pub fn get(scope: Scope<'_>, db: Database, ctx: HebiContext) -> hebi::Result<Value<'_>> {
    let key = scope.param::<String>(0)?;
    let value = db.get_hebi_data(ctx.channel_id, &key).map_err(|err| {
        error!("DB error: {err}");
        hebi::Error::User("Database error".into())
    })?;

    value.into_value(scope.global())
}

#[instrument(name = "hebi.db.get", skip_all)]
pub fn set(scope: Scope<'_>, db: Database, ctx: HebiContext) -> hebi::Result<()> {
    let key = scope.param::<String>(0)?;
    let value = scope.param::<String>(1)?;
    db.set_hebi_data(ctx.channel_id, &key, &value)
        .map_err(|err| {
            error!("DB error: {err}");
            hebi::Error::User("Database error".into())
        })?;

    Ok(())
}

#[instrument(name = "hebi.db.remove", skip_all)]
pub fn remove(scope: Scope<'_>, db: Database, ctx: HebiContext) -> hebi::Result<()> {
    let key = scope.param::<String>(0)?;
    db.remove_hebi_data(ctx.channel_id, &key).map_err(|err| {
        error!("DB error: {err}");
        hebi::Error::User("Database error".into())
    })?;

    Ok(())
}
