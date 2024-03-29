use async_trait::async_trait;
use axum::extract::{FromRequestParts, Query, State};
use axum::response::Redirect;
use axum_extra::extract::cookie::{Cookie, Key, SameSite};
use axum_extra::extract::PrivateCookieJar;
use chrono::{Duration, Utc};
use dashmap::DashMap;
use http::request::Parts;
use http::StatusCode;
use passwords::PasswordGenerator;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use std::{collections::HashMap, env};
use twitch_irc::login::{TokenStorage, UserAccessToken};

use crate::api::error::ApiError;
use crate::api::state::AppState;
use crate::command_handler::twitch_api;
use crate::command_handler::twitch_api::helix::HelixApi;
use crate::database::models::User;
use crate::{
    command_handler::{discord_api::DiscordApi, spotify_api::SpotifyApi, CommandHandler},
    database::{
        models::{UserData, WebSession},
        Database,
    },
    platform::UserIdentifier,
};

const TWITCH_AUTH_SCOPES: &[&str] = &[""];
const TWITCH_MANAGE_SCOPES: &[&str] = &[
    "channel:read:predictions",
    "channel:read:redemptions",
    "channel:manage:redemptions",
];
const DISCORD_SCOPES: &str = "identify";
const SPOTIFY_SCOPES: &[&str] = &["user-read-playback-state", "user-read-recently-played"];

const TWITCH_BOT_SCOPES: &[&str] = &[
    "chat:read",
    "chat:edit",
    "whispers:read",
    "whispers:edit",
    "channel:moderate",
    "moderator:manage:banned_users",
];

type StateStorage = State<Arc<DashMap<String, String>>>;

#[derive(Deserialize)]
pub struct Authenticateparams {
    pub redirect_to: Option<String>,
}

#[derive(Deserialize)]
pub struct RedirectParams {
    pub code: String,
    pub state: Option<String>,
}

pub async fn authenticate_twitch(
    state_storage: StateStorage,
    Query(Authenticateparams { redirect_to }): Query<Authenticateparams>,
) -> Redirect {
    tracing::info!("Authenticating with Twitch...");

    let token = generate_state_token();

    let client_id = twitch_api::get_client_id().expect("Twitch client ID not specified");

    let redirect_uri = AuthPlatform::Twitch.construct_uri(
        &client_id,
        &TWITCH_AUTH_SCOPES.join(" "),
        false,
        None,
        Some(&token),
    );

    state_storage.insert(token, redirect_to.unwrap_or_else(|| "/profile".to_string()));

    Redirect::to(&redirect_uri)
}

pub async fn admin_authenticate_twitch_bot(
    cmd: State<CommandHandler>,
    current_session: WebSession,
    state_storage: StateStorage,
    Query(Authenticateparams { redirect_to }): Query<Authenticateparams>,
) -> Result<Redirect, (StatusCode, &'static str)> {
    if let Ok(Some(admin_user)) = cmd.db.get_admin_user() {
        if admin_user.id == current_session.user_id {
            tracing::info!("Authenticating the bot (Twitch):");

            let client_id = twitch_api::get_client_id().expect("Twitch client ID not specified");

            let token = generate_state_token();

            let uri = AuthPlatform::Twitch.construct_uri(
                &client_id,
                &TWITCH_BOT_SCOPES.join("%20"),
                true,
                Some("bot"),
                Some(&token),
            );

            state_storage.insert(token, redirect_to.unwrap_or_else(|| "/profile".to_string()));

            tracing::info!("{}", uri);

            Ok(Redirect::to(&uri))
        } else {
            Err((StatusCode::UNAUTHORIZED, "Not admin user!"))
        }
    } else {
        Err((StatusCode::UNAUTHORIZED, "Admin user not configured!"))
    }
}

pub async fn authenticate_twitch_manage(
    state_storage: StateStorage,
    Query(Authenticateparams { redirect_to }): Query<Authenticateparams>,
) -> Redirect {
    let client_id = twitch_api::get_client_id().expect("Twitch client ID not specified");

    let token = generate_state_token();

    let uri = AuthPlatform::Twitch.construct_uri(
        &client_id,
        &TWITCH_MANAGE_SCOPES.join("%20"),
        true,
        Some("manage"),
        Some(&token),
    );

    state_storage.insert(token, redirect_to.unwrap_or_else(|| "/profile".to_string()));

    tracing::info!("{}", uri);

    Redirect::to(&uri)
}

pub async fn twitch_manage_redirect(
    cmd: State<CommandHandler>,
    Query(RedirectParams { code, state: _ }): Query<RedirectParams>,
    client: State<Client>,
    user: User,
) -> Result<Redirect, ApiError> {
    let twitch_user_id = user.twitch_id.ok_or(ApiError::InvalidUser)?;

    let mut user_credentials = cmd.db.make_twitch_credentials(twitch_user_id);

    let auth_response = trade_twitch_code(&client, &code).await?;

    let current = Utc::now();

    let token = UserAccessToken {
        access_token: auth_response.access_token,
        refresh_token: auth_response.refresh_token,
        created_at: current,
        expires_at: Some(current + Duration::seconds(auth_response.expires_in)),
    };

    user_credentials.update_token(&token).await?;

    Ok(Redirect::to("/profile"))
}

pub async fn twitch_redirect(
    cmd: State<CommandHandler>,
    client: State<Client>,
    mut jar: PrivateCookieJar,
    current_session: Option<WebSession>,
    state_storage: StateStorage,
    Query(RedirectParams { code, state }): Query<RedirectParams>,
) -> Result<(PrivateCookieJar, Redirect), (StatusCode, &'static str)> {
    let redirect_to = if let Some(state) = state {
        consume_state(&state, &state_storage)?
    } else {
        "/profile".to_string()
    };

    let auth_info = trade_twitch_code(&client, &code)
        .await
        .expect("Failed to get tokens");

    tracing::info!(
        "User authenticated with access token {}",
        auth_info.access_token
    );

    let helix_api = HelixApi::with_token(&auth_info.access_token)
        .await
        .expect("Failed to initialize Twitch API");

    let twitch_user = helix_api.get_self_user().await.unwrap();

    let user = cmd
        .db
        .get_or_create_user(&UserIdentifier::TwitchID(twitch_user.id))
        .expect("DB error");

    if let Some(web_session) = current_session {
        let current_user = cmd
            .db
            .get_user_by_id(web_session.user_id)
            .expect("DB Error")
            .unwrap();

        cmd.db.merge_users(current_user, user);
    } else {
        let cookie = create_user_session(&cmd.db, user.id, twitch_user.display_name);

        jar = jar.add(cookie);
    }

    Ok((jar, Redirect::to(&redirect_to)))
}

pub async fn admin_twitch_bot_redirect(
    cmd: State<CommandHandler>,
    client: State<Client>,
    Query(RedirectParams { code, state: _ }): Query<RedirectParams>,
    current_session: WebSession,
) -> Result<Redirect, (StatusCode, &'static str)> {
    if let Ok(Some(admin_user)) = cmd.db.get_admin_user() {
        if admin_user.id == current_session.user_id {
            let auth_response = trade_twitch_code(&client, &code)
                .await
                .expect("Failed to get Twitch auth response");

            let current = Utc::now();

            cmd.db
                .set_auth("twitch_access_token", &auth_response.access_token)
                .expect("DB error");
            cmd.db
                .set_auth("twitch_refresh_token", &auth_response.refresh_token)
                .expect("DB error");

            cmd.db
                .set_auth("twitch_created_at", &current.to_rfc3339())
                .expect("DB error");

            let expires_at = current + Duration::seconds(auth_response.expires_in);

            cmd.db
                .set_auth("twitch_expires_at", &expires_at.to_rfc3339())
                .expect("DB error");

            tracing::info!("Successfully authenticated the bot and saved the token!");

            Ok(Redirect::to("/profile"))
        } else {
            Err((StatusCode::UNAUTHORIZED, "Not admin user!"))
        }
    } else {
        Err((StatusCode::UNAUTHORIZED, "Admin user not configured!"))
    }
}

async fn trade_twitch_code(
    client: &Client,
    code: &str,
) -> Result<TwitchAuthenticationResponse, anyhow::Error> {
    let client_id = twitch_api::get_client_id().expect("Twitch client ID not specified");
    let client_secret =
        twitch_api::get_client_secret().expect("Twitch client secret not specified");

    let redirect_uri = format!(
        "{}/authenticate/twitch/redirect",
        env::var("BASE_URL").expect("BASE_URL missing")
    );

    let params = [
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("code", code),
        ("grant_type", "authorization_code"),
        ("redirect_uri", &redirect_uri),
    ];

    let response = client
        .post("https://id.twitch.tv/oauth2/token")
        .form(&params)
        .send()
        .await
        .unwrap();

    tracing::info!("POST {}: {}", &response.url(), &response.status());

    if response.status().is_client_error() {
        panic!("Received auth error: {}", response.text().await.unwrap());
    }

    Ok(response.json::<TwitchAuthenticationResponse>().await?)
}

pub async fn authenticate_discord(
    state_storage: StateStorage,
    Query(Authenticateparams { redirect_to }): Query<Authenticateparams>,
) -> Redirect {
    tracing::info!("Authenticating with Discord...");

    let client_id = env::var("DISCORD_CLIENT_ID").expect("DISCORD_CLIENT_ID missing");

    let token = generate_state_token();

    let redirect_uri =
        AuthPlatform::Discord.construct_uri(&client_id, DISCORD_SCOPES, false, None, Some(&token));

    state_storage.insert(token, redirect_to.unwrap_or_else(|| "/profile".to_string()));

    Redirect::to(&redirect_uri)
}

pub async fn discord_redirect(
    client: State<Client>,
    cmd: State<CommandHandler>,
    mut jar: PrivateCookieJar,
    current_session: Option<WebSession>,
    state_storage: StateStorage,
    Query(RedirectParams { code, state }): Query<RedirectParams>,
) -> Result<(PrivateCookieJar, Redirect), (StatusCode, &'static str)> {
    let redirect_to = if let Some(state) = state {
        consume_state(&state, &state_storage)?
    } else {
        "/profile".to_string()
    };

    let db = &cmd.db;

    let mut payload = HashMap::new();

    payload.insert(
        "client_id",
        env::var("DISCORD_CLIENT_ID").expect("DISCORD_CLIENT_ID missing"),
    );
    payload.insert(
        "client_secret",
        env::var("DISCORD_CLIENT_SECRET").expect("DISCORD_CLIENT_SECRET missing"),
    );
    payload.insert("grant_type", "authorization_code".to_owned());
    payload.insert("code", code);
    payload.insert(
        "redirect_uri",
        format!(
            "{}/authenticate/discord/redirect",
            env::var("BASE_URL").expect("BASE_URL missing")
        ),
    );

    let response = client
        .post("https://discord.com/api/oauth2/token")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&payload)
        .send()
        .await
        .expect("Discord API error");

    tracing::info!("POST {}: {}", response.url(), response.status());

    let auth_info = response
        .json::<DiscordAuthenticationResponse>()
        .await
        .expect("Discord JSON error");

    tracing::info!(
        "Authenticated Discord user with access_token {}",
        auth_info.access_token
    );

    let discord_api = DiscordApi::new(format!("Bearer {}", auth_info.access_token));

    let discord_user = discord_api
        .get_self_user()
        .await
        .expect("Discord API Error");

    let user = db
        .get_or_create_user(&UserIdentifier::DiscordID(discord_user.id.to_string()))
        .expect("DB Error");

    if let Some(web_session) = current_session {
        let current_user = cmd
            .db
            .get_user_by_id(web_session.user_id)
            .expect("DB Error")
            .unwrap();

        cmd.db.merge_users(current_user, user);
    } else {
        let cookie = create_user_session(db, user.id, discord_user.name);

        jar = jar.add(cookie);
    }

    Ok((jar, Redirect::to(&redirect_to)))
}

pub async fn authenticate_spotify(_session: WebSession) -> Redirect {
    let client_id = env::var("SPOTIFY_CLIENT_ID").expect("SPOTIFY_CLIENT_ID missing");

    Redirect::to(&AuthPlatform::Spotify.construct_uri(
        &client_id,
        &SPOTIFY_SCOPES.join("%20"),
        false,
        None,
        None,
    ))
}

pub async fn spotify_redirect(
    Query(RedirectParams { code, state: _ }): Query<RedirectParams>,
    cmd: State<CommandHandler>,
    session: WebSession,
) -> Redirect {
    let db = &cmd.db;

    let client_id = env::var("SPOTIFY_CLIENT_ID").expect("SPOTIFY_CLIENT_ID missing");
    let client_secret = env::var("SPOTIFY_CLIENT_SECRET").expect("SPOTIFY_CLIENT_SECRET missing");

    let redirect_uri = format!(
        "{}/authenticate/spotify/redirect",
        env::var("BASE_URL").expect("BASE_URL missing")
    );

    let auth = SpotifyApi::get_tokens(&code, &client_id, &client_secret, &redirect_uri)
        .await
        .expect("Spotify API Error");

    db.set_user_data(
        &UserData {
            name: "spotify_access_token".to_string(),
            value: auth.access_token,
            public: false,
            user_id: session.user_id,
        },
        true,
    )
    .expect("DB Error");

    db.set_user_data(
        &UserData {
            name: "spotify_refresh_token".to_string(),
            value: auth.refresh_token,
            public: false,
            user_id: session.user_id,
        },
        true,
    )
    .expect("DB Error");

    Redirect::to("/profile")
}

fn create_user_session(db: &Database, user_id: u64, display_name: String) -> Cookie<'static> {
    let session_id = db
        .create_web_session(user_id, display_name)
        .expect("DB error");

    Cookie::build("session_id", session_id)
        .secure(true)
        .path("/")
        .http_only(false)
        .same_site(SameSite::Lax)
        .finish()
}

fn generate_state_token() -> String {
    PasswordGenerator {
        length: 16,
        numbers: true,
        lowercase_letters: true,
        uppercase_letters: false,
        symbols: false,
        spaces: false,
        exclude_similar_characters: false,
        strict: true,
    }
    .generate_one()
    .expect("Failed to generate token")
}

// Returns the associated redirect uri
fn consume_state(
    state: &str,
    state_storage: &StateStorage,
) -> Result<String, (StatusCode, &'static str)> {
    if let Some((_, redirect_to)) = state_storage.remove(state) {
        Ok(redirect_to)
    } else {
        Err((StatusCode::UNAUTHORIZED, "State token not found"))
    }
}

#[derive(serde::Deserialize)]
struct TwitchAuthenticationResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

#[derive(serde::Deserialize)]
pub struct DiscordAuthenticationResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: String,
    pub scope: String,
}

#[derive(PartialEq, Eq)]
pub enum AuthPlatform {
    Twitch,
    Discord,
    Spotify,
}

impl AuthPlatform {
    fn get_name(&self) -> &str {
        match self {
            Self::Spotify => "spotify",
            Self::Twitch => "twitch",
            Self::Discord => "discord",
        }
    }

    fn get_auth_uri(&self) -> &str {
        match self {
            Self::Spotify => "https://accounts.spotify.com/authorize",
            Self::Twitch => "https://id.twitch.tv/oauth2/authorize",
            Self::Discord => "https://discord.com/api/oauth2/authorize",
        }
    }

    pub fn construct_uri(
        &self,
        client_id: &str,
        scopes: &str,
        force_verify: bool,
        suffix: Option<&str>,
        state: Option<&str>,
    ) -> String {
        let mut redirect_uri = format!(
            "{}/authenticate/{}/redirect",
            env::var("BASE_URL").expect("BASE_URL missing"),
            self.get_name(),
        );

        if let Some(suffix) = suffix {
            redirect_uri.push('/');
            redirect_uri.push_str(suffix);
        }

        tracing::info!("Using redirect_uri {}", redirect_uri);

        format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&force_verify={}&state={}",
            self.get_auth_uri(),
            client_id,
            redirect_uri,
            scopes,
            force_verify,
            state.unwrap_or_default(),
        )
    }
}

#[async_trait]
impl FromRequestParts<AppState> for WebSession {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let cookie_jar = PrivateCookieJar::<Key>::from_request_parts(parts, &state.secret_key)
            .await
            .unwrap();

        match cookie_jar.get("session_id") {
            Some(session_id) => match state
                .cmd
                .db
                .get_web_session(session_id.value())
                .expect("DB Error")
            {
                Some(web_session) => Ok(web_session),
                None => Err(StatusCode::UNAUTHORIZED),
            },
            None => Err(StatusCode::UNAUTHORIZED),
        }
    }
}

#[async_trait]
impl FromRequestParts<AppState> for User {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let session = WebSession::from_request_parts(parts, state).await?;

        // TODO handle this
        Ok(state
            .cmd
            .db
            .get_user_by_id(session.user_id)
            .expect("DB error")
            .expect("Invalid web session"))
    }
}
