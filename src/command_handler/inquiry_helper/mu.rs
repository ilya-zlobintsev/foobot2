use handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, Output, RenderContext, RenderError,
};
use mu::{EvalError, Mu};

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
            Err(err) => {
                let err_text = match err {
                    EvalError::Parse(err) => format!("{err:?}"),
                    EvalError::Runtime(err) => format!("{err:?}"),
                };
                Err(RenderError::new(format!(
                    "Failed to eval mu script {err_text}"
                )))
            }
        }
    }
}
