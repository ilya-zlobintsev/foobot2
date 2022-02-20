use handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, Output, RenderContext, RenderError,
};
use reqwest::Client;
use rhai::{Dynamic, Engine, EvalAltResult};

pub struct RhaiHelper {
    engine: Engine,
}

impl Default for RhaiHelper {
    fn default() -> Self {
        let mut helper = Self {
            engine: Engine::new(),
        };

        helper.engine.register_result_fn("get", get);
        helper.engine.register_result_fn("get_json", get_json);

        helper
    }
}

fn get(url: &str) -> Result<String, Box<EvalAltResult>> {
    let client = Client::new();

    let runtime = tokio::runtime::Handle::current();

    match runtime.block_on(client.get(url).send()) {
        Ok(response) => {
            if response.status().is_success() {
                let text = runtime.block_on(response.text()).unwrap();

                Ok(text)
            } else {
                Err(format!("Response status {}", response.status()).into())
            }
        }
        Err(e) => Err(e.to_string().into()),
    }
}

fn get_json(url: &str) -> Result<Dynamic, Box<EvalAltResult>> {
    let client = Client::new();

    let runtime = tokio::runtime::Handle::current();

    match runtime.block_on(client.get(url).send()) {
        Ok(response) => {
            if response.status().is_success() {
                let json = runtime
                    .block_on(response.json())
                    .map_err(|e| EvalAltResult::from(&format!("deserialization error: {}", e)))?;

                Ok(json)
            } else {
                Err(format!("Response status {}", response.status()).into())
            }
        }
        Err(e) => Err(e.to_string().into()),
    }
}

impl HelperDef for RhaiHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'reg, 'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let params = h
            .params()
            .iter()
            .map(|param| param.render())
            .collect::<Vec<String>>()
            .join(" ");

        match self.engine.eval::<Dynamic>(&params) {
            Ok(result) => match result.into_string() {
                Ok(s) => {
                    out.write(&s)?;
                    Ok(())
                }
                Err(e) => Err(RenderError::new(format!(
                    "Return value is of type {}, expected string",
                    e
                ))),
            },
            Err(e) => Err(RenderError::new(format!(
                "Failed to eval rhai script: {}",
                e
            ))),
        }
    }
}
