use std::thread;

use handlebars::{Context, Handlebars, Helper, HelperDef, RenderContext, RenderError, ScopedJson};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    database::{models::User, Database},
    platform::{ExecutionContext, UserIdentifier},
};

use super::{spotify_api::SpotifyApi, twitch_api::TwitchApi};

#[derive(Serialize, Deserialize)]
pub struct InquiryContext {
    pub user: User,
    pub execution_context: ExecutionContext,
    pub arguments: Vec<String>,
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
    pub twitch_api: Option<TwitchApi>,
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

        let runtime = tokio::runtime::Handle::current();

        let db = self.db.clone();
        let twitch_api = self.twitch_api.clone();

        match thread::spawn(move || {
            let user_id = match context.arguments.first() {
                Some(arg) => {
                    let user_identifier = runtime
                        .block_on(UserIdentifier::from_string(arg, twitch_api.as_ref()))
                        .map_err(|_| RenderError::new("invalid user"))?;

                    db.get_user(&user_identifier)
                        .expect("DB Error")
                        .ok_or_else(|| RenderError::new("invalid user"))?
                        .id
                }
                None => context.user.id,
            };

            let access_token = db
                .get_spotify_access_token(user_id)
                .map_err(|e| RenderError::new(format!("DB Error: {}", e.to_string())))?
                .ok_or_else(|| RenderError::new("not configured for user"))?;
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
