pub mod models;
mod schema;

use std::{
    collections::HashMap,
    env,
    fmt::Display,
    sync::{Arc, RwLock},
    time::Duration,
};

use self::models::*;
use crate::{
    command_handler::spotify_api::SpotifyApi,
    database::schema::*,
    platform::{ChannelIdentifier, UserIdentifier},
};
use diesel::mysql::MysqlConnection;
use diesel::{
    r2d2::{self, ConnectionManager, Pool},
    sql_query,
};
use diesel::{
    sql_types::{BigInt, Unsigned},
    ConnectionError,
};
use diesel::{EqAll, QueryDsl};
use diesel::{ExpressionMethods, RunQueryDsl};
use passwords::PasswordGenerator;
use reqwest::Client;
use rocket::figment::providers::Data;
use tokio::time;

use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

const BUILTIN_COMMANDS: &'static [&'static str] = &[
    "ping", "commands", "cmd", "command", "addcmd", "debug", "delcmd", "merge", "showcmd",
    "checkcmd",
];

#[derive(Clone)]
pub struct Database {
    conn_pool: Pool<ConnectionManager<MysqlConnection>>,
    web_sessions_cache: Arc<RwLock<HashMap<String, WebSession>>>,
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

        let web_sessions_cache = Arc::new(RwLock::new(HashMap::new()));

        Ok(Self {
            conn_pool,
            web_sessions_cache,
        })
    }

    pub fn start_cron(&self) {
        let conn_pool = self.conn_pool.clone();
        let web_sessions_cache = self.web_sessions_cache.clone();

        tokio::spawn(async move {
            loop {
                time::sleep(Duration::from_secs(3600)).await;

                tracing::info!("Clearing web sessions cache");

                let mut web_sessions_cache =
                    web_sessions_cache.write().expect("Failed to lock cache");

                web_sessions_cache.clear();
            }
        });

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

                let client_id = env::var("SPOTIFY_CLIENT_ID").expect("SPOTIFY_CLIENT_ID missing");
                let client_secret =
                    env::var("SPOTIFY_CLIENT_SECRET").expect("SPOTIFY_CLIENT_SECRET missing");

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
                            tracing::info!("Refreshed Spotify token for user {}", user_id);

                            diesel::update(
                                user_data::table
                                    .filter(user_data::name.eq_all("spotify_access_token"))
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
                            tracing::warn!("Error refreshing Spotify token: {}", e.to_string())
                        }
                    }
                }

                if refresh_in == None {
                    refresh_in = Some(3600);
                }

                tracing::info!("Completed! Next refresh in {} seconds", refresh_in.unwrap());

                time::sleep(Duration::from_secs(refresh_in.unwrap())).await;
            }
        });
    }

    pub fn get_channels(&self) -> Result<Vec<Channel>, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        channels::table.order(channels::id).load(&mut conn)
    }

    pub fn get_channel(
        &self,
        channel_identifier: &ChannelIdentifier,
    ) -> Result<Channel, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        let query = channels::table.into_boxed();

        // Doing .load().iter().next() looks nicer than doing .first() and then mapping NotFoundError to None
        match query
            .filter(channels::platform.eq_all(channel_identifier.get_platform_name()))
            .filter(channels::channel.eq_all(channel_identifier.get_channel()))
            .load(&mut conn)?
            .into_iter()
            .next()
        {
            Some(channel) => Ok(channel),
            None => {
                let new_channel = NewChannel {
                    platform: channel_identifier.get_platform_name(),
                    channel: &channel_identifier.get_channel(),
                };

                diesel::insert_into(channels::table)
                    .values(new_channel)
                    .execute(&mut conn)
                    .expect("Failed to create channel");

                self.get_channel(&channel_identifier)
            }
        }
    }

    pub fn get_channel_by_id(
        &self,
        channel_id: u64,
    ) -> Result<Option<Channel>, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        Ok(channels::table
            .filter(channels::id.eq_all(channel_id))
            .load(&mut conn)?
            .into_iter()
            .next())
    }

    pub fn get_channels_amount(&self) -> Result<i64, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        channels::table.count().get_result(&mut conn)
    }

    pub fn get_command(
        &self,
        channel_identifier: &ChannelIdentifier,
        command: &str,
    ) -> Result<Option<Command>, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        Ok(commands::table
            .filter(
                commands::channel_id.eq_any(
                    channels::table
                        .filter(channels::platform.eq_all(channel_identifier.get_platform_name()))
                        .filter(channels::channel.eq_all(channel_identifier.get_channel()))
                        .select(channels::id),
                ),
            )
            .filter(commands::name.eq_all(command))
            .load::<Command>(&mut conn)?
            .into_iter()
            .next())
    }

    pub fn get_commands(&self, channel_id: u64) -> Result<Vec<Command>, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        commands::table
            .filter(commands::channel_id.eq_all(channel_id))
            .load::<Command>(&mut conn)
    }

    pub fn add_command(
        &self,
        channel_identifier: &ChannelIdentifier,
        command_name: &str,
        command_action: &str,
    ) -> Result<(), DatabaseError> {
        let channel = self.get_channel(channel_identifier)?;

        self.add_command_to_channel(channel, command_name, command_action)
    }

    pub fn add_command_to_channel(
        &self,
        channel: Channel,
        command_name: &str,
        command_action: &str,
    ) -> Result<(), DatabaseError> {
        match BUILTIN_COMMANDS.contains(&command_name) {
            false => {
                let mut conn = self.conn_pool.get().unwrap();

                diesel::insert_into(commands::table)
                    .values(&NewCommand {
                        name: command_name,
                        action: command_action,
                        permissions: None,
                        channel_id: channel.id,
                    })
                    .execute(&mut conn)?;

                Ok(())
            }
            true => Err(DatabaseError::InvalidValue),
        }
    }

    pub fn add_command_to_channel_id(
        &self,
        channel_id: u64,
        command_name: &str,
        command_action: &str,
    ) -> Result<(), DatabaseError> {
        let channel = self.get_channel_by_id(channel_id)?.unwrap();

        self.add_command_to_channel(channel, command_name, command_action)
    }

    pub fn delete_command(
        &self,
        channel_identifier: &ChannelIdentifier,
        command_name: &str,
    ) -> Result<(), diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        diesel::delete(
            commands::table
                .filter(
                    commands::channel_id.eq_any(
                        channels::table
                            .filter(
                                channels::platform.eq_all(channel_identifier.get_platform_name()),
                            )
                            .filter(channels::channel.eq_all(channel_identifier.get_channel()))
                            .select(channels::id),
                    ),
                )
                .filter(commands::name.eq_all(command_name)),
        )
        .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_user(
        &self,
        user_identifier: &UserIdentifier,
    ) -> Result<Option<User>, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        let query = users::table.into_boxed();

        let query = match user_identifier {
            UserIdentifier::TwitchID(user_id) => query.filter(users::twitch_id.eq(Some(user_id))),
            UserIdentifier::DiscordID(user_id) => query.filter(users::discord_id.eq(Some(user_id))),
        };

        Ok(query.load(&mut conn)?.into_iter().next())
    }

    pub fn get_user_by_id(&self, user_id: u64) -> Result<Option<User>, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        Ok(users::table
            .filter(users::id.eq_all(user_id))
            .load(&mut conn)?
            .into_iter()
            .next())
    }

    pub fn get_or_create_user(
        &self,
        user_identifier: &UserIdentifier,
    ) -> Result<User, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        match self.get_user(&user_identifier)? {
            Some(user) => Ok(user),
            None => {
                let new_user = match &user_identifier {
                    UserIdentifier::TwitchID(user_id) => NewUser {
                        twitch_id: Some(&user_id),
                        discord_id: None,
                    },
                    UserIdentifier::DiscordID(user_id) => NewUser {
                        twitch_id: None,
                        discord_id: Some(&user_id),
                    },
                };

                diesel::insert_into(users::table)
                    .values(new_user)
                    .execute(&mut conn)
                    .expect("Failed to save new user");

                Ok(self.get_user(&user_identifier)?.unwrap())
            }
        }
    }

    pub fn merge_users(&self, mut user: User, other: User) -> User {
        let mut conn = self.conn_pool.get().unwrap();

        sql_query("REPLACE INTO user_data(user_id, name, value) SELECT ?, name, value FROM user_data WHERE user_id = ?").bind::<Unsigned<BigInt>, _>(user.id).bind::<Unsigned<BigInt>, _>(other.id).execute(&mut conn).expect("Failed to run replace query");

        diesel::delete(&other)
            .execute(&mut conn)
            .expect("Failed to delete");

        user.merge(other);

        diesel::update(users::table.filter(users::id.eq_all(user.id)))
            .set(&user)
            .execute(&mut conn)
            .expect("Failed to update");

        user
    }

    fn get_user_data_value(
        &self,
        user_id: u64,
        key: &str,
    ) -> Result<Option<String>, diesel::result::Error> {
        let mut conn = self.conn_pool.get().unwrap();

        Ok(user_data::table
            .filter(user_data::user_id.eq_all(user_id))
            .filter(user_data::name.eq_all(key))
            .select(user_data::value)
            .load(&mut conn)?
            .into_iter()
            .next())
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

    pub fn set_lastfm_token(&self, user_id: u64, token: String) -> Result<(), DatabaseError> {
        self.set_user_data(
            &UserData {
                name: "lastfm_token".to_string(),
                value: token,
                public: false,
                user_id,
            },
            true,
        )?;

        Ok(())
    }

    pub fn get_location(&self, user_id: u64) -> Result<Option<String>, diesel::result::Error> {
        self.get_user_data_value(user_id, "location")
    }

    pub fn get_web_session(
        &self,
        session_id: &str,
    ) -> Result<Option<WebSession>, diesel::result::Error> {
        let cache = self
            .web_sessions_cache
            .read()
            .expect("Failed to lock cache");

        match &cache.get(session_id) {
            Some(session) => Ok(Some(session.clone().clone())),
            None => {
                drop(cache);

                let mut conn = self.conn_pool.get().unwrap();

                match web_sessions::table
                    .filter(web_sessions::session_id.eq_all(session_id))
                    .load::<WebSession>(&mut conn)?
                    .into_iter()
                    .next()
                {
                    Some(session) => {
                        let mut cache = self
                            .web_sessions_cache
                            .write()
                            .expect("Failed to lock cache");

                        cache.insert(session_id.to_owned(), session.clone());

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

impl Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                DatabaseError::DieselError(e) => format!("database error: {}", e),
                DatabaseError::InvalidValue => "invalid value".to_string(),
            }
        )
    }
}
