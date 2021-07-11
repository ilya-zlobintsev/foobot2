use std::env;
use std::thread::{self, sleep};
use std::time::Duration;

use handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, JsonRender, Output, RenderContext,
    RenderError,
};
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

use crate::database::{models::User, Database};
use crate::platform::UserIdentifier;

use super::lastfm_api::LastFMApi;
use super::{owm_api::OwmApi, spotify_api::SpotifyApi};

#[derive(Serialize, Deserialize)]
pub struct InquiryContext {
    pub user: User,
    pub arguments: Vec<String>,
}

pub fn args_helper(
    _: &Helper,
    _: &Handlebars,
    ctx: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let context = serde_json::from_value::<InquiryContext>(ctx.data().clone())
        .expect("Failed to get command context");

    out.write(&context.arguments.join(" "))?;

    Ok(())
}

pub fn song_helper(
    h: &Helper,
    hb: &Handlebars,
    ctx: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let params = h
        .params()
        .iter()
        .map(|param| param.render())
        .collect::<Vec<String>>()
        .join(" ");

    tracing::info!("Params: {}", params);

    // Curly braces are doubled for escaping
    if let Ok(lastfm_result) =
        hb.render_template_with_context(&format!("{{{{ lastfm {} }}}}", params), ctx)
    {
        out.write(&lastfm_result)?;
    } else if let Ok(spotify_result) =
        hb.render_template_with_context(&format!("{{{{ spotify {} }}}}", params), ctx)
    {
        out.write(&spotify_result)?;
    } else {
        out.write(&format!(
            "No music source configured! Go to {}/profile to connect an account.",
            env::var("BASE_URL").expect("BASE_URL missing")
        ))?;
    }

    Ok(())
}

pub struct WeatherHelper {
    pub db: Database,
    pub api: OwmApi,
}

impl HelperDef for WeatherHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'reg, 'rc>,
        _: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let context = serde_json::from_value::<InquiryContext>(ctx.data().clone())
            .expect("Failed to get command context");

        let runtime = tokio::runtime::Handle::current();

        let place = match h.params().len() {
            0 => self
                .db
                .get_location(context.user.id)
                .map_err(|e| RenderError::new(format!("DB Error: {}", e)))?
                .ok_or_else(|| RenderError::new("location not set"))?,
            _ => {
                tracing::trace!("Helper params: {:?}", h.params());

                h.params()
                    .iter()
                    .map(|item| item.render())
                    .collect::<Vec<String>>()
                    .join(" ")
            }
        };

        tracing::info!("Querying weather for {}", place);

        let api = self.api.clone();

        // All of this is needed to call async apis from a blocking function
        let weather = thread::spawn(move || runtime.block_on(api.get_current(&place)))
            .join()
            .unwrap()
            .map_err(|e| RenderError::new(e.to_string()))?;

        out.write(&format!(
            "{}, {}: {}°C",
            weather.name,
            weather.sys.country.unwrap_or_default(),
            weather.main.temp
        ))?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct SpotifyHelper {
    pub db: Database,
}

impl HelperDef for SpotifyHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        _: &Helper<'reg, 'rc>,
        _: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let context = serde_json::from_value::<InquiryContext>(ctx.data().clone())
            .expect("Failed to get command context");

        let runtime = tokio::runtime::Handle::current();

        let db = self.db.clone();

        match thread::spawn(move || {
            let user_id = match context.arguments.first() {
                Some(arg) => {
                    tracing::info!("Arg {}", arg);
                    let user_identifier = UserIdentifier::from_string(arg)
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
                        "Not configured for user! You can set up Spotify by going to {}/profile",
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

                let artist = playback
                    .item
                    .artists
                    .iter()
                    .map(|artist| artist.name.as_str())
                    .collect::<Vec<&str>>()
                    .join(" ");

                out.write(&format!(
                    "{} - {} [{}/{}]",
                    artist, playback.item.name, position, length
                ))
                .expect("Failed to write");
            }
            None => out.write("No song is currently playing")?,
        };

        Ok(())
    }
}

#[derive(Clone)]
pub struct LastFMHelper {
    pub db: Database,
    pub lastfm_api: LastFMApi,
}

impl HelperDef for LastFMHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'reg, 'rc>,
        _: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let context = serde_json::from_value::<InquiryContext>(ctx.data().clone())
            .expect("Failed to get command context");

        let runtime = tokio::runtime::Handle::current();

        let username = match h.param(0) {
            Some(param) => {
                tracing::info!("Param {:?}", param);

                if let Some(param) = param.relative_path() {
                    param.clone()
                } else {
                    param.render()
                }
            }
            None => self
                .db
                .get_lastfm_name(context.user.id)
                .expect("DB Error")
                .ok_or_else(|| RenderError::new("last.fm username not set!"))?,
        };

        let lastfm_api = self.lastfm_api.clone();

        let response =
            thread::spawn(move || runtime.block_on(lastfm_api.get_recent_tracks(&username)))
                .join()
                .unwrap()
                .map_err(|e| RenderError::new(format!("Last.FM Error: {}", e)))?;

        out.write(&match response.recenttracks.track.iter().find(|track| {
            if let Some(attr) = &track.attr {
                attr.nowplaying == "true"
            } else {
                false
            }
        }) {
            Some(current_track) => {
                format!("{} - {}", current_track.artist.text, current_track.name)
            }
            None => "No song is currently playing".to_string(),
        })?;

        Ok(())
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

pub fn sleep_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    _: &mut dyn Output,
) -> HelperResult {
    match h.params().get(0) {
        Some(duration) => {
            sleep(Duration::from_secs(
                duration.value().as_u64().expect("Invalid duration"),
            ));

            Ok(())
        }
        None => Err(RenderError::new("sleep error: no duration specified")),
    }
}
