use crate::{
    command_handler::{error::CommandError, ExecutionContext},
    platform::PlatformContext,
};

#[derive(Debug, Clone)]
pub struct HebiContext {
    pub channel_id: u64,
}

impl<P: PlatformContext> TryFrom<&ExecutionContext<'_, P>> for HebiContext {
    type Error = CommandError;

    fn try_from(ctx: &ExecutionContext<P>) -> Result<Self, Self::Error> {
        Ok(HebiContext {
            channel_id: ctx.channel_id.ok_or_else(|| {
                CommandError::InvalidArgument(
                    "Hebi executing outside of a channel context".to_owned(),
                )
            })?,
        })
    }
}
