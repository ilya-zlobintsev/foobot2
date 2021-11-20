use std::{collections::HashMap, env};

use chrono::{Duration, Utc};
use reqwest::Client;
use rocket::get;
use rocket::http::{Cookie, CookieJar, SameSite};
use rocket::response::{content::Html, status, Redirect};
use rocket::State;
use rocket_dyn_templates::Template;
use twitch_irc::login::UserAccessToken;

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

const TWITCH_BOT_SCOPES: &[&'static str] = &[
    "chat:read",
    "chat:edit",
    "whispers:read",
    "whispers:edit",
    "channel:moderate",
];

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
pub async fn authenticate_twitch() -> Redirect {
    tracing::info!("Authenticating with Twitch...");

    let client_id = TwitchApi::get_client_id().expect("Twitch client ID not specified");

    Redirect::to(AuthPlatform::Twitch.construct_uri(&client_id, &TWITCH_SCOPES.join(" "), false))
}

#[get("/twitch/bot")]

pub async fn authenticate_twitch_bot(
    cmd: &State<CommandHandler>,
    current_session: WebSession,
) -> Result<Redirect, status::Unauthorized<&'static str>> {
    if let Ok(Some(admin_user)) = cmd.db.get_admin_user() {
        if admin_user.id == current_session.user_id {
            tracing::info!("Authenticating the bot (Twitch):");

            let client_id = TwitchApi::get_client_id().expect("Twitch client ID not specified");

            let uri = AuthPlatform::Twitch.construct_uri(
                &client_id,
                &TWITCH_BOT_SCOPES.join("%20"),
                true,
            );

            tracing::info!("{}", uri);

            Ok(Redirect::to(uri))
        } else {
            Err(status::Unauthorized(Some("Not admin user!")))
        }
    } else {
        Err(status::Unauthorized(Some("Admin user not configured!")))
    }
}

#[get("/twitch/redirect?<code>")]
pub async fn twitch_redirect(
    cmd: &State<CommandHandler>,
    client: &State<Client>,
    code: &str,
    jar: &CookieJar<'_>,
    current_session: Option<WebSession>,
) -> Redirect {
    let auth_info = trade_twitch_code(client, code)
        .await
        .expect("Failed to get tokens");

    tracing::info!(
        "User authenticated with access token {}",
        auth_info.access_token
    );

    let twitch_api = TwitchApi::init(&auth_info.access_token)
        .await
        .expect("Failed to initialize Twitch API");

    let twitch_user = twitch_api.get_self_user().await.unwrap();

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

        jar.add_private(cookie);
    }

    Redirect::found("/profile")
}

#[get("/twitch/redirect/bot?<code>")]
pub async fn twitch_bot_redirect(
    cmd: &State<CommandHandler>,
    client: &State<Client>,
    code: &str,
    current_session: WebSession,
) -> Result<Redirect, status::Unauthorized<&'static str>> {
    if let Ok(Some(admin_user)) = cmd.db.get_admin_user() {
        if admin_user.id == current_session.user_id {
            let auth_response = trade_twitch_code(client, code)
                .await
                .expect("Failed to get Twitch auth response");

            let current = Utc::now();

            cmd.db
                .save_token(&UserAccessToken {
                    access_token: auth_response.access_token,
                    refresh_token: auth_response.refresh_token,
                    created_at: current,
                    expires_at: Some(current + Duration::seconds(auth_response.expires_in)),
                })
                .expect("Failed to save token");

            tracing::info!("Successfully authenticated the bot and saved the token!");

            Ok(Redirect::found("/profile"))
        } else {
            Err(status::Unauthorized(Some("Not admin user!")))
        }
    } else {
        Err(status::Unauthorized(Some("Admin user not configured!")))
    }
}

async fn trade_twitch_code(
    client: &Client,
    code: &str,
) -> Result<TwitchAuthenticationResponse, anyhow::Error> {
    let client_id = TwitchApi::get_client_id().expect("Twitch client ID not specified");
    let client_secret = TwitchApi::get_client_secret().expect("Twitch client secret not specified");

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

#[get("/discord")]
pub fn authenticate_discord() -> Redirect {
    tracing::info!("Authenticating with Discord...");

    let client_id = env::var("DISCORD_CLIENT_ID").expect("DISCORD_CLIENT_ID missing");

    Redirect::to(AuthPlatform::Discord.construct_uri(&client_id, DISCORD_SCOPES, false))
}

#[get("/discord/redirect?<code>")]
pub async fn discord_redirect(
    client: &State<Client>,
    cmd: &State<CommandHandler>,
    code: String,
    jar: &CookieJar<'_>,
    current_session: Option<WebSession>,
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

    let discord_api = DiscordApi::new(&format!("Bearer {}", auth_info.access_token));

    let discord_user = discord_api
        .get_self_user()
        .await
        .expect("Discord API Error");

    let user = db
        .get_or_create_user(&UserIdentifier::DiscordID(discord_user.id.0.to_string()))
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

        jar.add_private(cookie);
    }

    Redirect::found("/profile")
}

#[get("/spotify")]
pub fn authenticate_spotify(_session: WebSession) -> Redirect {
    let client_id = env::var("SPOTIFY_CLIENT_ID").expect("SPOTIFY_CLIENT_ID missing");

    Redirect::to(AuthPlatform::Spotify.construct_uri(
        &client_id,
        &SPOTIFY_SCOPES.join("%20"),
        false,
    ))
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

#[derive(PartialEq)]
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

    pub fn construct_uri(&self, client_id: &str, scopes: &str, force_verify: bool) -> String {
        let mut redirect_uri = format!(
            "{}/authenticate/{}/redirect",
            env::var("BASE_URL").expect("BASE_URL missing"),
            self.get_name(),
        );

        // force_verify is used when authenticating the bot
        if force_verify && self == &Self::Twitch {
            redirect_uri.push_str("/bot");
        }

        tracing::info!("Using redirect_uri {}", redirect_uri);

        format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&force_verify={}",
            self.get_auth_uri(),
            client_id,
            redirect_uri,
            scopes,
            force_verify
        )
    }
}
