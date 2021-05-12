use rocket_contrib::templates::handlebars::{
    Context, Handlebars, Helper, HelperDef, RenderContext, RenderError, ScopedJson,
};
use serde_json::json;
use serde::{Serialize, Deserialize};

use crate::{database::models::User, platform::ExecutionContext};

#[derive(Serialize, Deserialize, Debug)]
pub struct InquiryContext {
    pub execution_context: ExecutionContext,
    pub user: User,
}

#[derive(Clone, Copy)]
pub struct ContextHelper;

impl HelperDef for ContextHelper {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        _: &Helper<'reg, 'rc>,
        _: &'reg Handlebars,
        ctx: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<Option<ScopedJson<'reg, 'rc>>, RenderError> {
        let inquiry_context: InquiryContext = serde_json::from_value(ctx.data().clone())?;

        Ok(Some(ScopedJson::Derived(json!({
            "user_id": inquiry_context.user.id,
            "channel": inquiry_context.execution_context.channel.get_channel(),
            "permissions": inquiry_context.execution_context.permissions.to_string(),
        }))))
    }
}
