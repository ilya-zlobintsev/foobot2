use handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, Output, RenderContext, RenderError,
};
use mu_runtime::{value::object::Registry, Handle, Isolate, Value};

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
        let emit_ctx = mu_emit::Context::new();
        let mut vm = Isolate::new(Handle::alloc(Registry::new()));

        let input = h
            .params()
            .iter()
            .map(|param| param.render())
            .collect::<Vec<String>>()
            .join(" ");

        let module = mu_syntax::parse(&input).unwrap();
        let module = mu_emit::emit(&emit_ctx, "code", &module).unwrap();
        let main = module.main();
        let result = vm.call(Value::object(main), &[], Value::none());

        match result {
            Ok(value) => {
                write!(out, "{value}")?;
                Ok(())
            }
            Err(err) => Err(RenderError::new(
                format!("Failed to eval mu script: {err}",),
            )),
        }
    }
}
