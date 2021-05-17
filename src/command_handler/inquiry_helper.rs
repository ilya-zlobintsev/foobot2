use std::thread;

use rocket_contrib::templates::handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, Output, RenderContext, RenderError,
    ScopedJson,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::database::{models::User, Database};

use super::spotify_api::SpotifyApi;

#[derive(Serialize, Deserialize, Debug)]
pub struct InquiryContext {
    pub user: User,
}

#[derive(Clone)]
pub struct ContextHelper;

impl HelperDef for ContextHelper {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        _: &Helper<'reg, 'rc>,
        _: &'reg Handlebars,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<Option<ScopedJson<'reg, 'rc>>, RenderError> {
        Ok(Some(ScopedJson::Derived(json!({
            "a": 1,
            "b": 2,
        }))))
    }
}

#[derive(Clone)]
pub struct SpotifyHelper {
    pub db: Database,
}

impl HelperDef for SpotifyHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        _: &Helper,
        _: &Handlebars,
        context: &Context,
        _: &mut RenderContext,
        out: &mut dyn Output,
    ) -> HelperResult {
        let context = serde_json::from_value::<InquiryContext>(context.data().clone())
            .expect("Failed to get command context");

        Ok(
            match self
                .db
                .get_spotify_access_token(context.user.id)
                .map_err(|e| RenderError::new(format!("DB Error: {}", e.to_string())))?
            {
                Some(access_token) => {
                    let runtime = tokio::runtime::Handle::current();

                    match thread::spawn(move || {
                        let spotify_api = SpotifyApi::new(&access_token);

                        runtime
                            .block_on(spotify_api.get_current_song())
                            .map_err(|e| {
                                RenderError::new(format!("Spotify API Error: {}", e.to_string()))
                            })
                    })
                    .join()
                    .unwrap()?
                    {
                        Some(song) => out.write(&song)?,
                        None => out.write("no song is currently plaing")?,
                    }
                }
                None => {
                    out.write("not configured for given user!")?;
                }
            },
        )
    }
}
