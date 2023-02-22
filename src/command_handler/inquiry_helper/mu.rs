use handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, Output, RenderContext, RenderError,
};
use mu::Mu;

#[derive(Default)]
pub struct MuHandler;

impl HelperDef for MuHandler {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'reg, 'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let mu = Mu::new();

        let input = h
            .params()
            .iter()
            .map(|param| param.render())
            .collect::<Vec<String>>()
            .join(" ");

        match mu.eval::<String>(&input) {
            Ok(value) => {
                write!(out, "{value}")?;
                Ok(())
            }
            Err(_) => Err(RenderError::new(format!(
                "Failed to eval mu script (cannot give an error right now due to mu changes)",
            ))),
        }
    }
}
