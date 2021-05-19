use std::{collections::HashMap, env};

use reqwest::Client;
use rocket::{State, get, http::{Cookie, CookieJar, SameSite}, response::{Redirect, content::Html}};
use rocket_contrib::templates::Template;

use crate::{
    command_handler::{discord_api::DiscordApi, spotify_api::SpotifyApi, twitch_api::TwitchApi},
    database::{models::UserData, Database},
    platform::UserIdentifier,
    web::template_context::LayoutContext,
};

use super::template_context::{AuthInfo, AuthenticateContext};

const TWITCH_SCOPES: &[&'static str] = &["user:read:email"];
const DISCORD_SCOPES: &'static str = "identify";
const SPOTIFY_SCOPES: &[&'static str] = &["user-read-playback-state", "user-read-recently-played"];

#[get("/")]
pub async fn index(db: &State<Database>, jar: &CookieJar<'_>) -> Html<Template> {
    Html(Template::render(
        "authenticate",
        &AuthenticateContext {
            parent_context: LayoutContext::new(db, jar),
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
pub async fn authenticate_twitch(twitch_api: &State<TwitchApi>) -> Redirect {
    tracing::info!("Authenticating with Twitch...");

    let client_id = twitch_api.get_client_id();

    let redirect_uri = format!(
        "{}/authenticate/twitch/redirect",
        env::var("BASE_URL").expect("BASE_URL missing")
    );

    tracing::info!("Using redirect_uri={}", redirect_uri);

    Redirect::to(format!("https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}", client_id, redirect_uri, TWITCH_SCOPES.join(" ")))
}

#[get("/twitch/redirect?<code>")]
pub async fn twitch_redirect(
    db: &State<Database>,
    twitch_api: &State<TwitchApi>,
    client: &State<Client>,
    code: &str,
    jar: &CookieJar<'_>,
) -> Redirect {
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
        .get_or_create_user(UserIdentifier::TwitchID(twitch_user.id))
        .expect("DB error");

    let cookie = create_user_session(db, user.id, twitch_user.display_name);

    jar.add_private(cookie);

    Redirect::found("/profile")
}

#[get("/discord")]
pub fn authenticate_discord() -> Redirect {
    tracing::info!("Authenticating with Discord...");

    let client_id = env::var("DISCORD_CLIENT_ID").expect("DISCORD_CLIENT_ID missing");

    let redirect_uri = format!(
        "{}/authenticate/discord/redirect",
        env::var("BASE_URL").expect("BASE_URL missing")
    );

    Redirect::to(format!("https://discord.com/api/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}", client_id, redirect_uri, DISCORD_SCOPES))
}

#[get("/discord/redirect?<code>")]
pub async fn discord_redirect(
    client: &State<Client>,
    db: &State<Database>,
    code: String,
    jar: &CookieJar<'_>,
) -> Redirect {
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
        .get_or_create_user(UserIdentifier::DiscordID(discord_user.id))
        .expect("DB Error");

    let cookie = create_user_session(db, user.id, discord_user.username);

    jar.add_private(cookie);

    Redirect::found("/profile")
}

#[get("/spotify")]
pub fn authenticate_spotify(db: &State<Database>, jar: &CookieJar<'_>) -> Redirect {
    match AuthInfo::new(&db, jar) {
        Some(_) => {
            let client_id = env::var("SPOTIFY_CLIENT_ID").expect("SPOTIFY_CLIENT_ID missing");

            let redirect_uri = format!(
                "{}/authenticate/spotify/redirect",
                env::var("BASE_URL").expect("BASE_URL missing")
            );
            Redirect::to(format!("https://accounts.spotify.com/authorize?client_id={}&response_type=code&redirect_uri={}&scope={}", client_id, redirect_uri, SPOTIFY_SCOPES.join("%20")))
        }
        None => Redirect::to("/authenticate"),
    }
}

#[get("/spotify/redirect?<code>")]
pub async fn spotify_redirect(code: &str, db: &State<Database>, jar: &CookieJar<'_>) -> Redirect {
    match AuthInfo::new(&db, jar) {
        Some(auth_info) => {
            let client_id = env::var("SPOTIFY_CLIENT_ID").expect("SPOTIFY_CLIENT_ID missing");
            let client_secret =
                env::var("SPOTIFY_CLIENT_SECRET").expect("SPOTIFY_CLIENT_SECRET missing");

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
                    user_id: auth_info.user_id,
                },
                true,
            )
            .expect("DB Error");

            db.set_user_data(
                &UserData {
                    name: "spotify_refresh_token".to_string(),
                    value: auth.refresh_token,
                    public: false,
                    user_id: auth_info.user_id,
                },
                true,
            )
            .expect("DB Error");

            Redirect::to("/profile")
        }
        None => Redirect::to("/authenticate"),
    }
}

#[get("/spotify/disconnect")]
pub fn disconnect_spotify(jar: &CookieJar<'_>, db: &State<Database>) -> Redirect {
    match AuthInfo::new(db, jar) {
        Some(auth_info) => {
            db.remove_user_data(auth_info.user_id, "spotify_access_token").expect("DB Error");
            db.remove_user_data(auth_info.user_id, "spotify_refresh_token").expect("DB Error");
            
            Redirect::to("/profile")
        }
        None => Redirect::to("/authenticate"),
    }
}

fn create_user_session(
    db: &State<Database>,
    user_id: u64,
    display_name: String,
) -> Cookie<'static> {
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
