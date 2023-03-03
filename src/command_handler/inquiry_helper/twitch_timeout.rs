use crate::{command_handler::platform_handler::TwitchApi, platform::ChannelIdentifier};
use handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, Output, RenderContext, RenderError,
};

use super::InquiryContext;

pub struct TwitchTimeoutHelper {
    pub twitch_api: TwitchApi,
}

impl HelperDef for TwitchTimeoutHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper,
        _: &Handlebars,
        ctx: &Context,
        _: &mut RenderContext,
        _: &mut dyn Output,
    ) -> HelperResult {
        let mut params = collect_params(h).into_iter();
        debug!("Collected params {params:?}");

        let name = params
            .next()
            .ok_or_else(|| RenderError::new("user name not specified"))?;

        let duration = params
            .next()
            .ok_or_else(|| RenderError::new("timeout duration not specified"))?;

        let length: i32 = duration
            .parse()
            .map_err(|_| RenderError::new("duration is not an integer"))?;

        debug!("Timing out user {name} for duration {length}");

        let context = serde_json::from_value::<InquiryContext>(ctx.data().clone())
            .expect("Failed to get command context");

        let broadcaster_id = match context.channel {
            ChannelIdentifier::TwitchChannel((id, _)) => id,
            _ => {
                return Err(RenderError::new(
                    "timeout cannot be used outside of Twitch!",
                ));
            }
        };

        let api = self.twitch_api.clone();

        let runtime = tokio::runtime::Handle::current();

        runtime
            .block_on(async move {
                api.helix_api
                    .ban_user_by_name(&broadcaster_id, &name, Some(length))
                    .await
            })
            .map_err(|e| {
                tracing::warn!("{:?}", e);
                RenderError::new("Failed to timeout user")
            })?;

        Ok(())
    }
}

fn collect_params(h: &Helper) -> Vec<String> {
    h.params()
        .iter()
        .flat_map(|param| {
            let value = match param.relative_path() {
                Some(path) => path.to_owned(),
                None => param.render(),
            };
            value
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
}
