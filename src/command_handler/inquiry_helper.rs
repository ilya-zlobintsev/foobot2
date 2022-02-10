use std::env;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, JsonRender, Output, RenderContext,
    RenderError, ScopedJson,
};
use rand::{thread_rng, Rng};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::time::sleep;
use twitch_irc::login::RefreshingLoginCredentials;

use crate::database::{models::User, Database};
use crate::platform::{ChannelIdentifier, UserIdentifier};

use super::finnhub_api::FinnhubApi;
use super::lastfm_api::LastFMApi;
use super::lingva_api::LingvaApi;
use super::platform_handler::PlatformHandler;
use super::twitch_api::TwitchApi;
use super::{owm_api::OwmApi, spotify_api::SpotifyApi};

#[derive(Serialize, Deserialize)]
pub struct InquiryContext {
    pub user: User,
    pub arguments: Vec<String>,
    pub display_name: String,
    pub channel: ChannelIdentifier,
}

pub struct TwitchUserHelper {
    pub twitch_api: TwitchApi<RefreshingLoginCredentials<Database>>,
}

impl HelperDef for TwitchUserHelper {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'reg, 'rc>,
        _: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<handlebars::ScopedJson<'reg, 'rc>, RenderError> {
        tracing::info!("{:?}", h.param(0));

        let param = h
            .param(0)
            .ok_or_else(|| RenderError::new("user not specified"))?;

        let user = match param.relative_path() {
            Some(path) => path.to_owned(),
            None => param.render(),
        };

        let mut logins = Vec::new();
        let mut ids = Vec::new();

        match user.is_empty() {
            false => logins.push(user),
            true => {
                let context = serde_json::from_value::<InquiryContext>(ctx.data().clone())
                    .expect("Failed to get command context");

                match context.user.twitch_id {
                    Some(twitch_id) => ids.push(twitch_id),
                    None => return Err(RenderError::new("user not specified")),
                }
            }
        }

        let runtime = tokio::runtime::Handle::current();

        let twitch_api = self.twitch_api.clone();

        let users_response = runtime
            .block_on(twitch_api.helix_api.get_users(
                Some(&logins.iter().map(|u| u.as_str()).collect::<Vec<&str>>()),
                Some(&ids.iter().map(|u| u.as_str()).collect::<Vec<&str>>()),
            ))
            .map_err(|e| RenderError::new(e.to_string()))?;

        let user = users_response
            .first()
            .ok_or_else(|| RenderError::new("user not found"))?;

        tracing::debug!("Twitch user: {:?}", user);

        Ok(ScopedJson::Derived(serde_json::to_value(user).unwrap()))
    }
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
    let mut params = h
        .params()
        .iter()
        .map(|param| param.render())
        .collect::<Vec<String>>()
        .join(" ");

    tracing::info!("Params: {}", params);

    if !params.is_empty() {
        params = format!("\"{}\"", params);
    }

    // Curly braces are doubled for escaping
    match hb.render_template_with_context(&format!("{{{{ lastfm {} }}}}", params), ctx) {
        Ok(lastfm_result) => {
            out.write(&lastfm_result)?;
        }
        Err(e) => {
            tracing::info!("Last.FM Error: {}", e);

            match hb.render_template_with_context(&format!("{{{{ spotify {} }}}}", params), ctx) {
                Ok(spotify_result) => out.write(&spotify_result)?,
                Err(e) => {
                    tracing::info!("Spotify Error: {}", e);

                    return Err(RenderError::new(format!(
                        "No music source configured! Go to {}/profile to connect an account.",
                        env::var("BASE_URL").expect("BASE_URL missing")
                    )));
                }
            }
        }
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
        let weather = runtime
            .block_on(api.get_current(&place))
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

fn get_spotify_api(db: Database, ctx: &Context, h: &Helper) -> Result<SpotifyApi, RenderError> {
    let context = serde_json::from_value::<InquiryContext>(ctx.data().clone())
        .expect("Failed to get command context");

    let user_id = match h.param(0) {
        Some(param) => {
            tracing::info!("Spotify param: {:?}", param);
            let user_identifier = UserIdentifier::from_string(&param.render())
                .map_err(|_| RenderError::new("invalid user"))?;

            db.get_user(&user_identifier)
                .expect("DB Error")
                .ok_or_else(|| RenderError::new("invalid user"))?
                .id
        }
        None => context.user.id,
    };

    tracing::info!("Looking for spotify token for user ID {}", user_id);

    let access_token = db
        .get_spotify_access_token(user_id)
        .map_err(|e| RenderError::new(format!("DB Error: {}", e)))?
        .ok_or_else(|| {
            RenderError::new(format!(
                "Not configured for user! You can set up Spotify by going to {}/profile",
                env::var("BASE_URL").unwrap()
            ))
        })?;

    Ok(SpotifyApi::new(&access_token))
}

#[derive(Clone)]
pub struct SpotifyHelper {
    pub db: Database,
}

impl HelperDef for SpotifyHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'reg, 'rc>,
        _: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let spotify_api = get_spotify_api(self.db.clone(), ctx, h)?;

        let runtime = tokio::runtime::Handle::current();

        match runtime
            .block_on(spotify_api.get_current_song())
            .map_err(|e| RenderError::new(format!("Spotify API Error: {}", e)))?
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
pub struct SpotifyPlaylistHelper {
    pub db: Database,
}

impl HelperDef for SpotifyPlaylistHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'reg, 'rc>,
        _: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let spotify_api = get_spotify_api(self.db.clone(), ctx, h)?;

        let runtime = tokio::runtime::Handle::current();

        match runtime
            .block_on(spotify_api.get_current_song())
            .map_err(|e| RenderError::new(format!("Spotify API Error: {}", e)))?
        {
            Some(playback) => {
                let response = match playback.context {
                    Some(context) => context.external_urls.spotify,
                    None => "not currently listening to a playlist".to_string(),
                };

                out.write(&response).unwrap();
            }
            None => out.write("No song is currently playing")?,
        };

        Ok(())
    }
}

#[derive(Clone)]
pub struct SpotifyLastHelper {
    pub db: Database,
}

impl HelperDef for SpotifyLastHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'reg, 'rc>,
        _: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let spotify_api = get_spotify_api(self.db.clone(), ctx, h)?;

        let runtime = tokio::runtime::Handle::current();

        let last_played = runtime
            .block_on(spotify_api.get_recently_played())
            .map_err(|e| RenderError::new(format!("Spotify API Error: {}", e)))?;

        out.write(&last_played)?;

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

        let user_id = match h.param(0) {
            Some(param) => {
                tracing::info!("Last.FM param: {:?}", param);
                let user_identifier = UserIdentifier::from_string(&param.render())
                    .map_err(|_| RenderError::new("invalid user"))?;

                self.db
                    .get_user(&user_identifier)
                    .expect("DB Error")
                    .ok_or_else(|| RenderError::new("invalid user"))?
                    .id
            }
            None => context.user.id,
        };

        let username = self
            .db
            .get_lastfm_name(user_id)
            .expect("DB Error")
            .ok_or_else(|| RenderError::new("last.fm username not set!"))?;

        let lastfm_api = self.lastfm_api.clone();

        let response = runtime
            .block_on(lastfm_api.get_recent_tracks(&username))
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
            let runtime = tokio::runtime::Handle::current();

            runtime.block_on(sleep(Duration::from_secs(
                duration.value().as_u64().expect("Invalid duration"),
            )));

            Ok(())
        }
        None => Err(RenderError::new("sleep error: no duration specified")),
    }
}

impl HelperDef for LingvaApi {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper,
        _: &Handlebars,
        _: &Context,
        _: &mut RenderContext,
        out: &mut dyn Output,
    ) -> HelperResult {
        let raw_params = h
            .params()
            .iter()
            .map(|param| match param.relative_path() {
                Some(path) => path.to_owned(),
                None => param.render(),
            })
            .collect::<Vec<String>>()
            .join(" ");

        let mut params = raw_params.split_whitespace().collect::<Vec<&str>>();

        // The join into split is needed to fix inconsistencies when calling the command with
        // `(args)` as opposed to passing the arguments to the helper directly

        let mut source = String::from("auto");
        let mut target = String::from("en");

        params.retain(|param| {
            if let Some(source_lang) = param.strip_prefix("from:") {
                source = source_lang.to_string();

                false
            } else if let Some(target_lang) = param.strip_prefix("to:") {
                target = target_lang.to_string();

                false
            } else {
                true
            }
        });

        let text = params.join(" ");

        tracing::info!("Translating text {}", text);

        let rt = Handle::current();

        match rt.block_on(self.translate(&source, &target, &text)) {
            Ok(translation) => out.write(&translation)?,
            Err(e) => out.write(&format!("error translating: {}", e))?,
        }

        Ok(())
    }
}

impl HelperDef for FinnhubApi {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper,
        _: &Handlebars,
        _: &Context,
        _: &mut RenderContext,
        out: &mut dyn Output,
    ) -> HelperResult {
        let param = h
            .param(0)
            .ok_or_else(|| RenderError::new("symbol not specified!"))?;

        let symbol = match param.relative_path() {
            Some(path) => path.to_owned(),
            None => param.render(),
        };

        let rt = Handle::current();

        match rt.block_on(self.quote(&symbol)) {
            Ok(quote) => {
                out.write(&format!(
                    "{} ({})",
                    quote.current_price,
                    quote.percent_change.unwrap_or(0.0)
                ))?;

                Ok(())
            }
            Err(e) => Err(RenderError::new(e.to_string())),
        }
    }
}

pub struct HttpHelper {
    client: Client,
}

impl HttpHelper {
    pub fn init() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl HelperDef for HttpHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper,
        _: &Handlebars,
        _: &Context,
        _: &mut RenderContext,
        out: &mut dyn Output,
    ) -> HelperResult {
        let url = h
            .params()
            .iter()
            .map(|param| match param.relative_path() {
                Some(path) => path.to_owned(),
                None => param.render(),
            })
            .collect::<Vec<String>>()
            .join("");

        tracing::info!("Making a request to: {}", url);

        let rt = Handle::current();

        let response = rt.block_on(self.client.get(url).send());

        match response {
            Ok(response) => {
                if response.status().is_success() {
                    match response.headers().get(http::header::CONTENT_TYPE) {
                        Some(content_type) => {
                            if content_type.to_str().unwrap().starts_with("text/plain") {
                                let text = rt
                                    .block_on(response.text())
                                    .unwrap_or_else(|_| "<empty text>".to_owned());

                                out.write(&text)?;

                                Ok(())
                            } else {
                                Err(RenderError::new("content type is not text/plain!"))
                            }
                        }
                        None => Err(RenderError::new("server did not return content type!")),
                    }
                } else {
                    out.write(&format!("HTTP status: {}", response.status()))?;

                    Ok(())
                }
            }
            Err(e) => Err(RenderError::new(e.to_string())),
        }
    }
}

pub fn username_helper(
    _: &Helper,
    _: &Handlebars,
    ctx: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let context = serde_json::from_value::<InquiryContext>(ctx.data().clone())
        .expect("Failed to get command context");

    out.write(&context.display_name)?;

    Ok(())
}

pub fn concat_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let len = h.params().len();

    if len != 0 {
        let result = h
            .params()
            .iter()
            .map(|param| param.value().render())
            .collect::<Vec<String>>()
            .join("");

        out.write(&result)?;
        Ok(())
    } else {
        Err(RenderError::new("missing items to choose from"))
    }
}

pub fn trim_matches_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let v = h
        .param(0)
        .map(|v| v.value().render())
        .ok_or(RenderError::new("param not found"))?;

    tracing::info!("Before trimming: {}", v);

    let x: &[_] = &vec!['@', ','];

    let v = v.trim_matches(x);

    tracing::info!("After trimming: {}", v);

    out.write(v)?;

    Ok(())
}

pub struct SetTempData {
    pub data: Arc<DashMap<String, String>>,
}

impl HelperDef for SetTempData {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper,
        _: &Handlebars,
        ctx: &Context,
        _: &mut RenderContext,
        _: &mut dyn Output,
    ) -> HelperResult {
        let raw_params = h
            .params()
            .iter()
            .map(|param| param.value().render())
            .collect::<Vec<String>>()
            .join(" ");

        let context = serde_json::from_value::<InquiryContext>(ctx.data().clone())
            .expect("Failed to get command context");

        let mut params = raw_params.split_whitespace().into_iter();

        let key = format!(
            "{}-{}-{}",
            context.channel.get_platform_name().unwrap_or_default(),
            context
                .channel
                .get_channel()
                .unwrap_or(&context.display_name),
            params
                .next()
                .ok_or_else(|| RenderError::new("key missing"))?
        );

        let value = params
            .next()
            .ok_or_else(|| RenderError::new("value missing"))?;

        tracing::info!("Set custom data {}: {}", key, value);

        self.data.insert(key, value.to_string());

        Ok(())
    }
}

pub struct GetTempData {
    pub data: Arc<DashMap<String, String>>,
}

impl HelperDef for GetTempData {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper,
        _: &Handlebars,
        ctx: &Context,
        _: &mut RenderContext,
        out: &mut dyn Output,
    ) -> HelperResult {
        let context = serde_json::from_value::<InquiryContext>(ctx.data().clone())
            .expect("Failed to get command context");

        let raw_params = h
            .params()
            .iter()
            .map(|param| param.value().render())
            .collect::<Vec<String>>()
            .join(" ");

        let mut params = raw_params.split_whitespace().into_iter();

        let response = match params.next() {
            Some(key) => {
                let key = format!(
                    "{}-{}-{}",
                    context.channel.get_platform_name().unwrap_or_default(),
                    context
                        .channel
                        .get_channel()
                        .unwrap_or(&context.display_name),
                    key,
                );

                match self.data.get(&key) {
                    Some(value) => value.value().to_string(),
                    None => String::new(),
                }
            }
            None => {
                let keys = self
                    .data
                    .iter()
                    .map(|e| e.key().as_str().to_string())
                    .collect::<Vec<String>>();

                if keys.is_empty() {
                    return Err(RenderError::new("No data"));
                }
                keys.join(", ")
            }
        };

        out.write(&response)?;

        Ok(())
    }
}

pub struct SayHelper {
    pub platform_handler: PlatformHandler,
}
impl HelperDef for SayHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper,
        _: &Handlebars,
        ctx: &Context,
        _: &mut RenderContext,
        _: &mut dyn Output,
    ) -> HelperResult {
        let params = h
            .params()
            .iter()
            .map(|param| param.value().render())
            .collect::<Vec<String>>()
            .join(" ");

        let context = serde_json::from_value::<InquiryContext>(ctx.data().clone())
            .expect("Failed to get command context");

        let runtime = tokio::runtime::Handle::current();

        let platform_handler = self.platform_handler.clone();

        runtime.spawn(async move {
            if let Err(e) = platform_handler
                .send_to_channel(context.channel, params)
                .await
            {
                tracing::warn!("Failed sending message from inqury context: {}", e);
            }
        });

        Ok(())
    }
}
