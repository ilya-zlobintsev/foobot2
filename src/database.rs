pub mod models;
mod schema;

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use self::models::*;
use crate::{
    database::schema::*,
    platform::{ChannelIdentifier, UserIdentifier},
};
use diesel::mysql::MysqlConnection;
use diesel::r2d2::{self, ConnectionManager, Pool};
use diesel::ConnectionError;
use diesel::{EqAll, QueryDsl};
use diesel::{ExpressionMethods, RunQueryDsl};
use passwords::PasswordGenerator;
use tokio::time;

embed_migrations!();

#[derive(Clone)]
pub struct Database {
    conn_pool: Pool<ConnectionManager<MysqlConnection>>,
    web_sessions_cache: Arc<RwLock<HashMap<String, WebSession>>>,
}

impl Database {
    pub fn connect(database_url: String) -> Result<Self, ConnectionError> {
        let manager = ConnectionManager::<MysqlConnection>::new(database_url);
        let conn_pool = r2d2::Pool::new(manager).expect("Failed to set up DB connection pool");

        embedded_migrations::run(&conn_pool.get().unwrap()).expect("Failed to run migrations");

        let web_sessions_cache = Arc::new(RwLock::new(HashMap::new()));

        Ok(Self {
            conn_pool,
            web_sessions_cache,
        })
    }

    pub fn start_cron(&self) {
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
    }

    pub fn get_channels(&self) -> Result<Vec<Channel>, diesel::result::Error> {
        let conn = self.conn_pool.get().unwrap();

        channels::table.order(channels::id).load(&conn)
    }

    pub fn get_channel(
        &self,
        channel_identifier: &ChannelIdentifier,
    ) -> Result<Channel, diesel::result::Error> {
        let conn = self.conn_pool.get().unwrap();

        let query = channels::table.into_boxed();

        // Doing .load().iter().next() looks nicer than doing .first() and then mapping NotFoundError to None
        match query
            .filter(channels::platform.eq_all(channel_identifier.get_platform_name()))
            .filter(channels::channel.eq_all(channel_identifier.get_channel()))
            .load(&conn)?
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
                    .execute(&conn)
                    .expect("Failed to create channel");

                self.get_channel(&channel_identifier)
            }
        }
    }

    pub fn get_channels_amount(&self) -> Result<i64, diesel::result::Error> {
        let conn = self.conn_pool.get().unwrap();

        channels::table.count().get_result(&conn)
    }

    pub fn get_command(
        &self,
        channel_identifier: &ChannelIdentifier,
        command: &str,
    ) -> Result<Option<Command>, diesel::result::Error> {
        let conn = self.conn_pool.get().unwrap();

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
            .load::<Command>(&conn)?
            .into_iter()
            .next())
    }

    pub fn get_commands(&self, channel_id: u64) -> Result<Vec<Command>, diesel::result::Error> {
        let conn = self.conn_pool.get().unwrap();

        commands::table
            .filter(commands::channel_id.eq_all(channel_id))
            .load::<Command>(&conn)
    }

    pub fn add_command(
        &self,
        channel_identifier: &ChannelIdentifier,
        command_name: &str,
        command_action: &str,
    ) -> Result<(), diesel::result::Error> {
        let conn = self.conn_pool.get().unwrap();

        // I couldn't figure out how to do this as a subquery
        let channel_id = self.get_channel(channel_identifier)?.id;

        diesel::insert_into(commands::table)
            .values(&NewCommand {
                name: command_name,
                action: command_action,
                permissions: None,
                channel_id,
            })
            .execute(&conn)?;

        Ok(())
    }

    pub fn delete_command(
        &self,
        channel_identifier: &ChannelIdentifier,
        command_name: &str,
    ) -> Result<(), diesel::result::Error> {
        let conn = self.conn_pool.get().unwrap();

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
        .execute(&conn)?;

        Ok(())
    }

    pub fn get_user(
        &self,
        user_identifier: &UserIdentifier,
    ) -> Result<Option<User>, diesel::result::Error> {
        let conn = self.conn_pool.get().unwrap();

        let query = users::table.into_boxed();

        let query = match user_identifier {
            UserIdentifier::TwitchID(user_id) => {
                query.filter(users::twitch_id.eq_all(Some(user_id)))
            }
            UserIdentifier::DiscordID(user_id) => {
                query.filter(users::discord_id.eq_all(Some(user_id)))
            }
        };

        Ok(query.load(&conn)?.into_iter().next())
    }

    pub fn get_or_create_user(
        &self,
        user_identifier: UserIdentifier,
    ) -> Result<User, diesel::result::Error> {
        let conn = self.conn_pool.get().unwrap();

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
                    .execute(&conn)
                    .expect("Failed to save new user");

                Ok(self.get_user(&user_identifier)?.unwrap())
            }
        }
    }

    pub fn merge_users(&self, mut user: User, other: User) -> Result<User, diesel::result::Error> {
        let conn = self.conn_pool.get().unwrap();

        diesel::update(user_data::table.filter(user_data::user_id.eq_all(other.id)))
            .set(user_data::user_id.eq_all(user.id))
            .execute(&conn)?;

        diesel::update(web_sessions::table.filter(web_sessions::user_id.eq_all(other.id)))
            .set(web_sessions::user_id.eq_all(user.id))
            .execute(&conn)?;

        diesel::delete(&other).execute(&conn)?;

        user.merge(other);

        diesel::update(users::table).set(&user).execute(&conn)?;

        Ok(user)
    }

    fn get_user_data_value(
        &self,
        user_id: u64,
        key: &str,
    ) -> Result<Option<String>, diesel::result::Error> {
        let conn = self.conn_pool.get().unwrap();

        Ok(user_data::table
            .filter(user_data::user_id.eq_all(user_id))
            .filter(user_data::name.eq_all(key))
            .select(user_data::value)
            .load(&conn)?
            .into_iter()
            .next())
    }

    pub fn get_spotify_access_token(
        &self,
        user_id: u64,
    ) -> Result<Option<String>, diesel::result::Error> {
        self.get_user_data_value(user_id, "spotify_access_token")
    }

    pub fn get_spotify_refresh_token(
        &self,
        user_id: u64,
    ) -> Result<Option<String>, diesel::result::Error> {
        self.get_user_data_value(user_id, "spotify_refresh_token")
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

                let conn = self.conn_pool.get().unwrap();

                match web_sessions::table
                    .filter(web_sessions::session_id.eq_all(session_id))
                    .load::<WebSession>(&conn)?
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
        let conn = self.conn_pool.get().unwrap();

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
            .execute(&conn)?;

        Ok(session.session_id)
    }
}
