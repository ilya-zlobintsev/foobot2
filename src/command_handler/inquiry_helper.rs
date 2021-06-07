use std::{env, thread::{self, sleep}, time::Duration};

use handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, JsonRender, Output, RenderContext,
    RenderError, ScopedJson,
};
use rand::{thread_rng, Rng};
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
    ) -> Result<ScopedJson<'reg, 'rc>, RenderError> {
        Ok(ScopedJson::Derived(json!({
            "a": 1,
            "b": 2,
        })))
    }
}

#[derive(Clone)]
pub struct SpotifyHelper {
    pub db: Database,
    pub twitch_api: Option<TwitchApi>,
}

impl HelperDef for SpotifyHelper {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        _: &Helper<'reg, 'rc>,
        _: &'reg Handlebars,
        context: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'reg, 'rc>, RenderError> {
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
                .ok_or_else(|| {
                    RenderError::new(format!(
                        "not configured for user! You can set up Spotify by going to {}/profile",
                        env::var("BASE_URL").unwrap()
                    ))
                })?;
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

                Ok(json!({
                    "artist": playback.item.artists.iter().map(|artist| artist.name.as_str()).collect::<Vec<&str>>().join(" "),
                    "song": playback.item.name,
                    "position": format!("{}/{}", position, length),
                    "playlist": match playback.context {
                        Some(context) => context.external_urls.spotify,
                        None => "".to_string(),
                    },
                }).into())
            }
            None => Ok(json!(null).into()),
        }
    }
}

pub fn random_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let len = h.params().len();

    if len != 0 {
        let mut rng = thread_rng();

        let param = h.params().get(rng.gen_range(0..len)).expect("RNG Error");

        out.write(param.value().render().as_ref())?;
        Ok(())
    } else {
        Err(RenderError::new("missing items to choose from"))
    }
}

pub fn sleep_helper(h: &Helper, _: &Handlebars, _: &Context, _: &mut RenderContext, out: &mut dyn Output) -> HelperResult {
    match h.params().get(0) {
        Some(duration) => {
            sleep(Duration::from_secs(duration.value().as_u64().expect("Invalid duration")));

            Ok(())
        }
        None => Err(RenderError::new("sleep error: no duration specified"))
    }
}