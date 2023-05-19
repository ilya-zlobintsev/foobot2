use handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, Output, RenderContext, RenderError,
};
use hebi::Hebi;

#[derive(Default)]
pub struct HebiHandler;

impl HelperDef for HebiHandler {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'reg, 'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let mut hebi = Hebi::new();

        let input = h
            .params()
            .iter()
            .map(|param| param.render())
            .collect::<Vec<String>>()
            .join(" ");

        match hebi.eval(&input) {
            Ok(value) => {
                write!(out, "{value}")?;
                Ok(())
            }
            Err(err) => Err(RenderError::new(format!(
                "Failed to eval hebi script {err}"
            ))),
        }
    }
}
