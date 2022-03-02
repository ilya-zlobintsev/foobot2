pub mod credentials;
pub mod models;
mod schema;

use std::env;
use std::fmt::Display;
use std::sync::Arc;
use std::time::Duration;

use self::credentials::Credentials;
use self::models::*;
use crate::command_handler::spotify_api::SpotifyApi;
use crate::database::schema::*;
use crate::platform::{ChannelIdentifier, UserIdentifier, UserIdentifierError};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use diesel::mysql::MysqlConnection;
use diesel::r2d2::{self, ConnectionManager, Pool};
use diesel::sql_types::{BigInt, Unsigned};
use diesel::{sql_query, EqAll, QueryDsl};
use diesel::{ConnectionError, OptionalExtension};
use diesel::{ExpressionMethods, RunQueryDsl};
use passwords::PasswordGenerator;

use reqwest::Client;
use tokio::time;

use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use twitch_irc::login::{TokenStorage, UserAccessToken};
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

const BUILTIN_COMMANDS: &[&str] = &[
    "ping", "commands", "cmd", "command", "addcmd", "debug", "delcmd", "merge", "showcmd",
    "checkcmd",
];

#[derive(Clone, Debug)]
pub struct Database {
    conn_pool: Pool<ConnectionManager<MysqlConnection>>,
    web_sessions_cache: Arc<DashMap<String, WebSession>>,
    users_cache: Arc<DashMap<u64, User>>,
    user_identifiers_cache: Arc<DashMap<UserIdentifier, u64>>, // Caches the user IDs
    prefixes_cache: Arc<DashMap<u64, Option<String>>>,
    channels_cache: Arc<DashMap<ChannelIdentifier, Channel>>,
}

impl Database {
    pub fn connect(database_url: String) -> Result<Self, ConnectionError> {
        let manager = ConnectionManager::<MysqlConnection>::new(&database_url);
        let conn_pool = r2d2::Pool::new(manager).expect("Failed to set up DB connection pool");

        conn_pool
            .get()
            .unwrap()
            .run_pending_migrations(MIGRATIONS)
            .expect("Failed to run migrations");

        let web_sessions_cache = Arc::new(DashMap::new());
        let users_cache = Arc::new(DashMap::new());
        let user_identifiers_cache = Arc::new(DashMap::new());
        let prefixes_cache = Arc::new(DashMap::new());
        let channels_cache = Arc::new(DashMap::new());

        Ok(Self {
            conn_pool,
            web_sessions_cache,
            users_cache,
            user_identifiers_cache,
            prefixes_cache,
            channels_cache,
        })
    }

    pub fn start_cron(&self) {
        let web_sessions_cache = self.web_sessions_cache.clone();
        let users_cache = self.users_cache.clone();
        let user_identifiers_cache = self.user_identifiers_cache.clone();

        tokio::spawn(async move {
            loop {
                time::sleep(Duration::from_secs(3600)).await;

                tracing::info!("Clearing caches");

                web_sessions_cache.clear();
                users_cache.clear();
                user_identifiers_cache.clear();
            }
        });

        {
            let conn_pool = self.conn_pool.clone();

            if let Ok(client_id) = env::var("SPOTIFY_CLIENT_ID") {
                if let Ok(client_secret) = env::var("SPOTIFY_CLIENT_SECRET") {
                    tokio::spawn(async move {
                        loop {
                            tracing::info!("Updating Spotify tokens...");

                            let mut conn = conn_pool.get().unwrap();

                            let refresh_tokens = user_data::table
                                .select((user_data::user_id, user_data::value))
                                .filter(user_data::name.eq_all("spotify_refresh_token"))
                                .load::<(u64, String)>(&mut conn)
                                .expect("DB Error");

                            let mut refresh_in = None;

                            let client = Client::new();

                            for (user_id, refresh_token) in refresh_tokens {
                                match SpotifyApi::update_token(
                                    &client,
                                    &client_id,
                                    &client_secret,
                                    &refresh_token,
                                )
                                .await
                                {
                                    Ok((access_token, expiration_time)) => {
                                        tracing::info!(
                                            "Refreshed Spotify token for user {}",
                                            user_id
                                        );

                                        diesel::update(
                                            user_data::table
                                                .filter(
                                                    user_data::name.eq_all("spotify_access_token"),
                                                )
                                                .filter(user_data::user_id.eq_all(user_id)),
                                        )
                                        .set(user_data::value.eq_all(access_token))
                                        .execute(&mut conn)
                                        .expect("DB Error");

                                        if refresh_in == None {
                                            refresh_in = Some(expiration_time);
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "Error refreshing Spotify token: {}",
                                            e.to_string()
                                        )
                                    }
                                }
                            }

                            if refresh_in == None {
                                refresh_in = Some(3600);
                            }

                            tracing::info!(
                                "Completed! Next refresh in {} seconds",
                                refresh_in.unwrap()
                            );

                            time::sleep(Duration::from_secs(refresh_in.unwrap())).await;
                        }
                    });
                }
            }
        }
    }

    pub fn get_channels(&self) -> Result<Vec<Channel>, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        channels::table.order(channels::id).load(&mut conn)
    }

    pub fn get_channel(
        &self,
        channel_identifier: &ChannelIdentifier,
    ) -> Result<Option<Channel>, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        if let Some(channel) = channel_identifier.get_channel() {
            if let Some(channel) = self.channels_cache.get(channel_identifier) {
                Ok(Some(channel.value().clone()))
            } else {
                let channel = channels::table
                    .filter(
                        channels::platform.eq_all(channel_identifier.get_platform_name().unwrap()),
                    )
                    .filter(channels::channel.eq_all(channel))
                    .first::<Channel>(&mut conn)
                    .optional()?;

                if let Some(channel) = &channel {
                    self.channels_cache
                        .insert(channel_identifier.clone(), channel.clone());
                }

                Ok(channel)
            }
        } else {
            Ok(None)
        }
    }

    pub fn get_or_create_channel(
        &self,
        channel_identifier: &ChannelIdentifier,
    ) -> Result<Option<Channel>, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        let query = channels::table.into_boxed();

        if let Some(channel) = channel_identifier.get_channel() {
            match query
                .filter(channels::platform.eq_all(channel_identifier.get_platform_name().unwrap()))
                .filter(channels::channel.eq_all(channel))
                .first(&mut conn)
                .optional()?
            {
                Some(channel) => Ok(Some(channel)),
                None => {
                    let new_channel = NewChannel {
                        platform: channel_identifier.get_platform_name().unwrap(),
                        channel,
                    };

                    diesel::insert_into(channels::table)
                        .values(new_channel)
                        .execute(&mut conn)
                        .expect("Failed to create channel");

                    self.get_or_create_channel(channel_identifier)
                }
            }
        } else {
            Ok(None)
        }
    }

    pub fn get_admin_user(&self) -> Result<Option<User>, DatabaseError> {
        match env::var("ADMIN_USER") {
            Ok(s) => {
                let admin_identifier = UserIdentifier::from_string(&s)?;

                Ok(self.get_user(&admin_identifier)?)
            }
            Err(_) => Ok(None),
        }
    }

    pub fn get_channel_by_id(
        &self,
        channel_id: u64,
    ) -> Result<Option<Channel>, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        channels::table
            .filter(channels::id.eq_all(channel_id))
            .first(&mut conn)
            .optional()
    }

    pub fn get_channels_amount(&self) -> Result<i64, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        channels::table.count().get_result(&mut conn)
    }

    pub fn get_command(
        &self,
        channel_identifier: &ChannelIdentifier,
        command: &str,
    ) -> Result<Option<Command>, DatabaseError> {
        let mut conn = self.conn_pool.get().unwrap();

        match channel_identifier.get_channel() {
            Some(channel) => Ok(commands::table
                .filter(
                    commands::channel_id.eq_any(
                        channels::table
                            .filter(
                                channels::platform
                                    .eq_all(channel_identifier.get_platform_name().unwrap()),
                            )
                            .filter(channels::channel.eq_all(channel))
                            .select(channels::id),
                    ),
                )
                .filter(commands::name.eq_all(command))
                .first(&mut conn)
                .optional()?),
            None => Ok(None),
        }
    }

    pub fn get_commands(&self, channel_id: u64) -> Result<Vec<Command>, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        commands::table
            .filter(commands::channel_id.eq_all(channel_id))
            .load::<Command>(&mut conn)
    }

    pub fn add_command_to_channel(
        &self,
        channel_identifier: &ChannelIdentifier,
        trigger: &str,
        action: &str,
    ) -> Result<(), DatabaseError> {
        let channel_id = self.get_or_create_channel(channel_identifier)?.unwrap().id;

        self.add_command(NewCommand {
            name: trigger,
            action,
            permissions: None,
            channel_id,
            cooldown: 5,
        })
    }

    fn add_command(&self, command: NewCommand) -> Result<(), DatabaseError> {
        match BUILTIN_COMMANDS.contains(&command.name) {
            false => {
                let mut conn = self.conn_pool.get().unwrap();

                diesel::insert_into(commands::table)
                    .values(&command)
                    .execute(&mut conn)?;

                Ok(())
            }
            true => Err(DatabaseError::InvalidValue),
        }
    }

    pub fn update_command(&self, command: NewCommand) -> Result<(), DatabaseError> {
        let mut conn = self.conn_pool.get().unwrap();

        diesel::replace_into(commands::table)
            .values(&command)
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn delete_command_from_channel(
        &self,
        channel_identifier: &ChannelIdentifier,
        command_name: &str,
    ) -> Result<(), DatabaseError> {
        let channel = self.get_or_create_channel(channel_identifier)?.unwrap();

        self.delete_command(channel.id, command_name)
    }

    pub fn delete_command(&self, channel_id: u64, command_name: &str) -> Result<(), DatabaseError> {
        let mut conn = self.conn_pool.get().unwrap();

        let affected = diesel::delete(
            commands::table
                .filter(commands::channel_id.eq(channel_id))
                .filter(commands::name.eq_all(command_name)),
        )
        .execute(&mut conn)?;

        match affected {
            0 => Err(DatabaseError::InvalidValue),
            _ => Ok(()),
        }
    }

    pub fn get_user(
        &self,
        user_identifier: &UserIdentifier,
    ) -> Result<Option<User>, diesel::result::Error> {
        match self.user_identifiers_cache.get(user_identifier) {
            Some(id) => self.get_user_by_id(*id),
            None => {
                let mut conn = self.conn_pool.get().unwrap();

                let query = users::table.into_boxed();

                let query = match user_identifier {
                    UserIdentifier::TwitchID(user_id) => {
                        query.filter(users::twitch_id.eq(Some(user_id)))
                    }
                    UserIdentifier::DiscordID(user_id) => {
                        query.filter(users::discord_id.eq(Some(user_id)))
                    }
                    UserIdentifier::TelegramId(id) => {
                        query.filter(users::telegram_id.eq(Some(id.to_string())))
                    }
                    UserIdentifier::IrcName(name) => query.filter(users::irc_name.eq(Some(name))),
                    UserIdentifier::IpAddr(addr) => {
                        query.filter(users::local_addr.eq(Some(addr.to_string())))
                    }
                };

                Ok(query.first::<User>(&mut conn).optional()?.map(|user| {
                    self.user_identifiers_cache
                        .insert(user_identifier.clone(), user.id);

                    user
                }))
            }
        }
    }

    pub fn get_user_by_id(&self, user_id: u64) -> Result<Option<User>, diesel::result::Error> {
        match self.users_cache.get(&user_id) {
            Some(user) => Ok(Some(user.clone())),
            None => {
                let mut conn = self.conn_pool.get().unwrap();

                match users::table
                    .filter(users::id.eq_all(user_id))
                    .first::<User>(&mut conn)
                    .optional()?
                {
                    Some(user) => {
                        tracing::debug!("Cached user {}", user_id);
                        self.users_cache.insert(user_id, user.clone());

                        Ok(Some(user))
                    }
                    None => Ok(None),
                }
            }
        }
    }

    pub fn get_or_create_user(
        &self,
        user_identifier: &UserIdentifier,
    ) -> Result<User, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        match self.get_user(user_identifier)? {
            Some(user) => Ok(user),
            None => {
                let new_user = match &user_identifier {
                    UserIdentifier::TwitchID(user_id) => NewUser {
                        twitch_id: Some(user_id),
                        ..Default::default()
                    },
                    UserIdentifier::DiscordID(user_id) => NewUser {
                        discord_id: Some(user_id),
                        ..Default::default()
                    },
                    UserIdentifier::IrcName(name) => NewUser {
                        irc_name: Some(&*name),
                        ..Default::default()
                    },
                    UserIdentifier::IpAddr(addr) => NewUser {
                        local_addr: Some(addr.to_string()),
                        ..Default::default()
                    },
                    UserIdentifier::TelegramId(id) => NewUser {
                        telegram_id: Some(id.to_string()),
                        ..Default::default()
                    }
                };

                diesel::insert_into(users::table)
                    .values(new_user)
                    .execute(&mut conn)
                    .expect("Failed to save new user");

                Ok(self.get_user(user_identifier)?.unwrap())
            }
        }
    }

    pub fn merge_users(&self, mut user: User, other: User) -> User {
        let mut conn = self.conn_pool.get().unwrap();

        self.users_cache.remove(&other.id);

        sql_query("REPLACE INTO user_data(user_id, name, value) SELECT ?, name, value FROM user_data WHERE user_id = ?").bind::<Unsigned<BigInt>, _>(user.id).bind::<Unsigned<BigInt>, _>(other.id).execute(&mut conn).expect("Failed to run replace query");

        diesel::delete(&other)
            .execute(&mut conn)
            .expect("Failed to delete");

        user.merge(other);

        diesel::update(users::table.filter(users::id.eq_all(user.id)))
            .set(&user)
            .execute(&mut conn)
            .expect("Failed to update");

        self.users_cache.remove(&user.id);

        self.user_identifiers_cache.clear();

        user
    }

    pub fn get_auth(&self, key: &str) -> Result<Option<String>, DatabaseError> {
        let mut conn = self.conn_pool.get().unwrap();

        Ok(auth::table
            .filter(auth::name.eq_all(key))
            .select(auth::value)
            .first(&mut conn)
            .optional()?
            .unwrap_or_default())
    }

    pub fn set_auth(&self, key: &str, value: &str) -> Result<(), DatabaseError> {
        let mut conn = self.conn_pool.get().unwrap();

        tracing::debug!("Setting auth: {} - {}", key, value);

        diesel::replace_into(auth::table)
            .values((auth::name.eq(key), auth::value.eq(value)))
            .execute(&mut conn)?;

        Ok(())
    }

    fn get_user_data_value(
        &self,
        user_id: u64,
        key: &str,
    ) -> Result<Option<String>, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        user_data::table
            .filter(user_data::user_id.eq_all(user_id))
            .filter(user_data::name.eq_all(key))
            .select(user_data::value)
            .first(&mut conn)
            .optional()
    }

    pub fn get_eventsub_redeem_action(
        &self,
        id: &str,
    ) -> Result<Option<String>, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        eventsub_triggers::table
            .filter(eventsub_triggers::id.eq_all(id))
            .select(eventsub_triggers::action)
            .first(&mut conn)
            .optional()
    }

    pub fn get_eventsub_triggers(&self) -> Result<Vec<EventSubTrigger>, DatabaseError> {
        let mut conn = self.conn_pool.get().unwrap();

        Ok(eventsub_triggers::table.load(&mut conn)?)
    }

    pub fn get_eventsub_triggers_for_broadcaster(
        &self,
        broadcaster_id: &str,
    ) -> Result<Vec<EventSubTrigger>, DatabaseError> {
        let mut conn = self.conn_pool.get().unwrap();

        Ok(eventsub_triggers::table
            .filter(eventsub_triggers::broadcaster_id.eq(broadcaster_id))
            .load(&mut conn)?)
    }
    pub fn set_user_data(
        &self,
        user_data: &UserData,
        overwrite: bool,
    ) -> Result<(), diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        match overwrite {
            true => diesel::replace_into(user_data::table)
                .values(user_data)
                .execute(&mut conn),
            false => diesel::insert_into(user_data::table)
                .values(user_data)
                .execute(&mut conn),
        }?;

        Ok(())
    }

    pub fn remove_user_data(&self, user_id: u64, data: &str) -> Result<(), diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        diesel::delete(
            user_data::table
                .filter(user_data::user_id.eq_all(user_id))
                .filter(user_data::name.eq_all(data)),
        )
        .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_spotify_access_token(
        &self,
        user_id: u64,
    ) -> Result<Option<String>, diesel::result::Error> {
        self.get_user_data_value(user_id, "spotify_access_token")
    }

    pub fn get_location(&self, user_id: u64) -> Result<Option<String>, diesel::result::Error> {
        self.get_user_data_value(user_id, "location")
    }

    pub fn get_lastfm_name(&self, user_id: u64) -> Result<Option<String>, DatabaseError> {
        Ok(self.get_user_data_value(user_id, "lastfm_name")?)
    }

    pub fn set_lastfm_name(&self, user_id: u64, name: &str) -> Result<(), DatabaseError> {
        Ok(self.set_user_data(
            &UserData {
                name: "lastfm_name".to_string(),
                value: name.to_string(),
                public: true,
                user_id,
            },
            true,
        )?)
    }

    pub fn get_web_session(
        &self,
        session_id: &str,
    ) -> Result<Option<WebSession>, diesel::result::Error> {
        match self.web_sessions_cache.get(session_id) {
            Some(session) => Ok(Some(session.clone())),
            None => {
                let mut conn = self.conn_pool.get().unwrap();

                match web_sessions::table
                    .filter(web_sessions::session_id.eq_all(session_id))
                    .first::<WebSession>(&mut conn)
                    .optional()?
                {
                    Some(session) => {
                        self.web_sessions_cache
                            .insert(session_id.to_owned(), session.clone());

                        tracing::debug!("Inserted session {} into cache", session_id);

                        Ok(Some(session))
                    }
                    None => Ok(None),
                }
            }
        }
    }

    /// Returns the session id
    pub fn create_web_session(
        &self,
        user_id: u64,
        username: String,
    ) -> Result<String, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        let session = WebSession {
            session_id: PasswordGenerator {
                length: 24,
                numbers: true,
                lowercase_letters: true,
                uppercase_letters: true,
                symbols: true,
                spaces: true,
                exclude_similar_characters: false,
                strict: true,
            }
            .generate_one()
            .unwrap(),
            user_id,
            username,
        };

        diesel::insert_into(web_sessions::table)
            .values(&session)
            .execute(&mut conn)?;

        Ok(session.session_id)
    }

    pub fn add_eventsub_trigger(&self, trigger: NewEventSubTrigger) -> Result<(), DatabaseError> {
        let mut conn = self.conn_pool.get().unwrap();

        diesel::insert_into(eventsub_triggers::table)
            .values(trigger)
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn delete_eventsub_trigger(&self, id: &str) -> Result<(), DatabaseError> {
        let mut conn = self.conn_pool.get().unwrap();

        diesel::delete(eventsub_triggers::table)
            .filter(eventsub_triggers::id.eq(id))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn update_eventsub_trigger_id(
        &self,
        old_id: &str,
        new_id: &str,
    ) -> Result<(), DatabaseError> {
        let mut conn = self.conn_pool.get().unwrap();

        diesel::update(eventsub_triggers::table)
            .filter(eventsub_triggers::id.eq(old_id))
            .set(eventsub_triggers::id.eq(new_id))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn make_twitch_credentials(&self, user_id: String) -> Credentials {
        Credentials {
            db: self.clone(),
            user_id,
        }
    }

    pub fn get_prefix(&self, channel_id: u64) -> Result<Option<String>, DatabaseError> {
        let mut conn = self.conn_pool.get().unwrap();

        match self.prefixes_cache.get(&channel_id) {
            Some(prefix_entry) => Ok(prefix_entry.value().clone()),
            None => {
                let prefix = prefixes::table
                    .filter(prefixes::channel_id.eq_all(channel_id))
                    .first::<Prefix>(&mut conn)
                    .optional()?
                    .map(|p| p.prefix);

                self.prefixes_cache.insert(channel_id, prefix.clone());

                Ok(prefix)
            }
        }
    }

    pub fn get_prefix_in_channel(
        &self,
        channel: &ChannelIdentifier,
    ) -> Result<Option<String>, DatabaseError> {
        if let Some(channel) = self.get_channel(channel)? {
            self.get_prefix(channel.id)
        } else {
            Ok(None)
        }
    }

    pub fn get_mirror_connections(&self) -> Result<Vec<MirrorConnection>, DatabaseError> {
        let mut conn = self.conn_pool.get().unwrap();

        Ok(mirror_connections::table.load(&mut conn)?)
    }

    pub fn create_mirror_connection(
        &self,
        connection: MirrorConnection,
    ) -> Result<(), DatabaseError> {
        let mut conn = self.conn_pool.get().unwrap();

        diesel::insert_into(mirror_connections::table)
            .values(&connection)
            .execute(&mut conn)?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum DatabaseError {
    DieselError(diesel::result::Error),
    InvalidValue,
}

impl From<diesel::result::Error> for DatabaseError {
    fn from(e: diesel::result::Error) -> Self {
        Self::DieselError(e)
    }
}

impl From<UserIdentifierError> for DatabaseError {
    fn from(_: UserIdentifierError) -> Self {
        Self::InvalidValue
    }
}

impl Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                DatabaseError::DieselError(e) => format!("Database error: {}", e),
                DatabaseError::InvalidValue => "Invalid value".to_string(),
            }
        )
    }
}

impl std::error::Error for DatabaseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

#[async_trait]
impl TokenStorage for Database {
    type LoadError = anyhow::Error;
    type UpdateError = anyhow::Error;

    async fn load_token(&mut self) -> Result<UserAccessToken, Self::LoadError> {
        let access_token = self.get_auth("twitch_access_token")?.unwrap_or_default();
        let refresh_token = self.get_auth("twitch_refresh_token")?.unwrap_or_default();

        let created_at = DateTime::from_utc(
            DateTime::parse_from_rfc3339(&self.get_auth("twitch_created_at")?.unwrap_or_default())?
                .naive_utc(),
            Utc,
        );

        let expires_at = match self.get_auth("twitch_expires_at")? {
            Some(date) => Some(DateTime::from_utc(
                DateTime::parse_from_rfc3339(&date)?.naive_utc(),
                Utc,
            )),
            None => None,
        };

        Ok(UserAccessToken {
            access_token,
            refresh_token,
            created_at,
            expires_at,
        })
    }

    async fn update_token(&mut self, token: &UserAccessToken) -> Result<(), Self::UpdateError> {
        tracing::info!("Refreshed Twitch token!");

        self.set_auth("twitch_access_token", &token.access_token)?;
        self.set_auth("twitch_refresh_token", &token.refresh_token)?;

        self.set_auth("twitch_created_at", &token.created_at.to_rfc3339())?;

        if let Some(expires_at) = token.expires_at {
            self.set_auth("twitch_expires_at", &expires_at.to_rfc3339())?;
        }

        Ok(())
    }
}
