use std::{collections::HashMap, env};

use reqwest::Client;
use rocket::{
    get,
    http::{Cookie, CookieJar, SameSite},
    response::{content::Html, Redirect},
    State,
};
use rocket_dyn_templates::Template;

use crate::{
    command_handler::{
        discord_api::DiscordApi, spotify_api::SpotifyApi, twitch_api::TwitchApi, CommandHandler,
    },
    database::{
        models::{UserData, WebSession},
        Database,
    },
    platform::UserIdentifier,
    web::template_context::LayoutContext,
};

use super::template_context::AuthenticateContext;

const TWITCH_SCOPES: &[&'static str] = &[""];
const DISCORD_SCOPES: &'static str = "identify";
const SPOTIFY_SCOPES: &[&'static str] = &["user-read-playback-state", "user-read-recently-played"];

#[get("/")]
pub async fn index(session: Option<WebSession>) -> Html<Template> {
    Html(Template::render(
        "authenticate",
        &AuthenticateContext {
            parent_context: LayoutContext::new_with_auth(session),
        },
    ))
}

#[get("/logout")]
pub async fn logout(jar: &CookieJar<'_>) -> Redirect {
    if let Some(session_cookie) = jar.get_private("session_id") {
        jar.remove_private(session_cookie);
    }

    Redirect::to("/")
}

#[get("/twitch")]
pub async fn authenticate_twitch(cmd: &State<CommandHandler>) -> Redirect {
    let twitch_api = cmd.twitch_api.as_ref().expect("Twitch not configured");

    tracing::info!("Authenticating with Twitch...");

    let client_id = twitch_api.get_client_id();

    Redirect::to(AuthPlatform::Twitch.construct_uri(client_id, &TWITCH_SCOPES.join(" ")))
}

#[get("/twitch/redirect?<code>")]
pub async fn twitch_redirect(
    cmd: &State<CommandHandler>,
    client: &State<Client>,
    code: &str,
    jar: &CookieJar<'_>,
) -> Redirect {
    let db = &cmd.db;
    let twitch_api = &cmd.twitch_api.as_ref().expect("Twitch not configured");

    let client_id = twitch_api.get_client_id();
    let client_secret = env::var("TWITCH_CLIENT_SECRET").expect("TWITCH_CLIENT_SECRET missing");

    let redirect_uri = format!(
        "{}/authenticate/twitch/redirect",
        env::var("BASE_URL").expect("BASE_URL missing")
    );

    let params = [
        ("client_id", client_id),
        ("client_secret", &client_secret),
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

    let auth_info = response
        .json::<TwitchAuthenticationResponse>()
        .await
        .expect("Failed to process twitch authentication response");

    tracing::info!(
        "User authenticated with access token {}",
        auth_info.access_token
    );

    let twitch_api = TwitchApi::init(&auth_info.access_token)
        .await
        .expect("Failed to initialize Twitch API");

    let twitch_user = twitch_api.get_self_user().await.unwrap();

    let user = db
        .get_or_create_user(&UserIdentifier::TwitchID(twitch_user.id))
        .expect("DB error");

    let cookie = create_user_session(db, user.id, twitch_user.display_name);

    jar.add_private(cookie);

    Redirect::found("/profile")
}

#[get("/discord")]
pub fn authenticate_discord() -> Redirect {
    tracing::info!("Authenticating with Discord...");

    let client_id = env::var("DISCORD_CLIENT_ID").expect("DISCORD_CLIENT_ID missing");

    Redirect::to(AuthPlatform::Discord.construct_uri(&client_id, DISCORD_SCOPES))
}

#[get("/discord/redirect?<code>")]
pub async fn discord_redirect(
    client: &State<Client>,
    cmd: &State<CommandHandler>,
    code: String,
    jar: &CookieJar<'_>,
) -> Redirect {
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

    let discord_api = DiscordApi::init(&auth_info.access_token);

    let discord_user = discord_api
        .get_self_user()
        .await
        .expect("Discord API Error");

    let user = db
        .get_or_create_user(&UserIdentifier::DiscordID(discord_user.id))
        .expect("DB Error");

    let cookie = create_user_session(db, user.id, discord_user.username);

    jar.add_private(cookie);

    Redirect::found("/profile")
}

#[get("/spotify")]
pub fn authenticate_spotify(cmd: &State<CommandHandler>, session: WebSession) -> Redirect {
    let db = &cmd.db;

    let client_id = env::var("SPOTIFY_CLIENT_ID").expect("SPOTIFY_CLIENT_ID missing");

    Redirect::to(AuthPlatform::Spotify.construct_uri(&client_id, &SPOTIFY_SCOPES.join("%20")))
}

#[get("/spotify/redirect?<code>")]
pub async fn spotify_redirect(
    code: &str,
    cmd: &State<CommandHandler>,
    session: WebSession,
) -> Redirect {
    let db = &cmd.db;

    let client_id = env::var("SPOTIFY_CLIENT_ID").expect("SPOTIFY_CLIENT_ID missing");
    let client_secret = env::var("SPOTIFY_CLIENT_SECRET").expect("SPOTIFY_CLIENT_SECRET missing");

    let redirect_uri = format!(
        "{}/authenticate/spotify/redirect",
        env::var("BASE_URL").expect("BASE_URL missing")
    );

    let auth = SpotifyApi::get_tokens(code, &client_id, &client_secret, &redirect_uri)
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

#[get("/spotify/disconnect")]
pub fn disconnect_spotify(session: WebSession, cmd: &State<CommandHandler>) -> Redirect {
    let db = &cmd.db;

    db.remove_user_data(session.user_id, "spotify_access_token")
        .expect("DB Error");
    db.remove_user_data(session.user_id, "spotify_refresh_token")
        .expect("DB Error");

    Redirect::to("/profile")
}

fn create_user_session(db: &Database, user_id: u64, display_name: String) -> Cookie<'static> {
    let session_id = db
        .create_web_session(user_id, display_name.to_string())
        .expect("DB error");

    Cookie::build("session_id", session_id)
        .secure(true)
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .finish()
}

#[derive(serde::Deserialize)]
struct TwitchAuthenticationResponse {
    pub access_token: String,
}

#[derive(serde::Deserialize)]
pub struct DiscordAuthenticationResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: String,
    pub scope: String,
}

enum AuthPlatform {
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

    pub fn construct_uri(&self, client_id: &str, scopes: &str) -> String {
        let redirect_uri = format!(
            "{}/authenticate/{}/redirect",
            env::var("BASE_URL").expect("BASE_URL missing"),
            self.get_name(),
        );

        tracing::info!("Using redirect_uri {}", redirect_uri);

        format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}",
            self.get_auth_uri(),
            client_id,
            redirect_uri,
            scopes
        )
    }
}
