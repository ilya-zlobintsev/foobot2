use std::env;

use rocket::{
    get,
    http::{Cookie, CookieJar},
    response::Redirect,
    State,
};
use rocket_contrib::templates::Template;

use crate::{
    command_handler::twitch_api::TwitchApi, database::Database, platform::UserIdentifier,
    web::template_context::LayoutContext,
};

use super::template_context::AuthenticateContext;

#[get("/")]
pub async fn index(db: &State<Database>, jar: &CookieJar<'_>) -> Template {
    Template::render(
        "authenticate",
        &AuthenticateContext {
            parent_context: LayoutContext::new(db, jar),
        },
    )
}

const SCOPES: &[&'static str] = &["user:read:email"];

#[get("/twitch")]
pub async fn authenticate_twitch(twitch_api: &State<TwitchApi>) -> Redirect {
    tracing::info!("Authenticating with Twitch...");

    let client_id = twitch_api.get_client_id();

    let redirect_uri = format!(
        "{}/authenticate/twitch/redirect",
        env::var("BASE_URL").expect("BASE_URL missing")
    );

    tracing::info!("Using redirect_uri={}", redirect_uri);

    Redirect::to(format!("https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}", client_id, redirect_uri, SCOPES.join(" ")))
}

#[get("/twitch/redirect?<code>")]
pub async fn twitch_redirect(
    db: &State<Database>,
    twitch_api: &State<TwitchApi>,
    code: &str,
    jar: &CookieJar<'_>,
) -> Redirect {
    let client = reqwest::Client::new();

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
        .get_user(UserIdentifier::TwitchID(twitch_user.id))
        .expect("DB error");

    let session_id = db
        .create_web_session(user.id, twitch_user.display_name)
        .expect("DB error");

    let mut cookie = Cookie::new("session_id", session_id);

    cookie.set_secure(true);

    jar.add_private(cookie);

    Redirect::to("/authenticate")
}

#[derive(serde::Deserialize)]
struct TwitchAuthenticationResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub scope: Vec<String>,
    pub expires_in: i64,
    pub token_type: String,
}
