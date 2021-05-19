use std::thread;

use rocket_contrib::templates::handlebars::{
    Context, Handlebars, Helper, HelperDef, RenderContext, RenderError, ScopedJson,
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

// TODO: figure out why the helper gets called twice when using "with" handlebars statements
impl HelperDef for SpotifyHelper {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        _: &Helper<'reg, 'rc>,
        _: &'reg Handlebars,
        context: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<Option<ScopedJson<'reg, 'rc>>, RenderError> {
        let context = serde_json::from_value::<InquiryContext>(context.data().clone())
            .expect("Failed to get command context");

        let access_token = self
            .db
            .get_spotify_access_token(context.user.id)
            .map_err(|e| RenderError::new(format!("DB Error: {}", e.to_string())))?
            .ok_or_else(|| RenderError::new("not configured for user"))?;

        let runtime = tokio::runtime::Handle::current();

        match thread::spawn(move || {
            let spotify_api = SpotifyApi::new(&access_token);

            runtime
                .block_on(spotify_api.get_current_song())
                .map_err(|e| RenderError::new(format!("Spotify API Error: {}", e.to_string())))
        })
        .join()
        .unwrap()?
        {
            Some(playback) => {
                let position = playback.progress_ms / 1000;
                let position = format!("{}:{:02}", position / 60, position % 60);

                let length = playback.item.duration_ms / 1000;
                let length = format!("{}:{:02}", length / 60, length % 60);

                Ok(Some(ScopedJson::Derived(json!({
                    "artist": playback.item.artists.iter().map(|artist| artist.name.as_str()).collect::<Vec<&str>>().join(" "),
                    "song": playback.item.name,
                    "position": format!("{}/{}", position, length),
                    "playlist": playback.context.external_urls.spotify,
                }))))
            }
            None => Ok(None),
        }
    }
}
