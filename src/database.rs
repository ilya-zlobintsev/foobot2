pub mod models;
mod schema;

use self::models::*;
use crate::{
    database::schema::channels,
    database::schema::commands,
    database::schema::users,
    platform::{ChannelIdentifier, UserIdentifier},
};
use diesel::mysql::MysqlConnection;
use diesel::r2d2::{self, ConnectionManager, Pool};
use diesel::ConnectionError;
use diesel::{EqAll, QueryDsl};
use diesel::{ExpressionMethods, RunQueryDsl};

embed_migrations!();

#[derive(Clone)]
pub struct Database {
    conn_pool: Pool<ConnectionManager<MysqlConnection>>,
}

impl Database {
    pub fn connect(database_url: String) -> Result<Self, ConnectionError> {
        let manager = ConnectionManager::<MysqlConnection>::new(database_url);
        let conn_pool = r2d2::Pool::new(manager).expect("Failed to set up DB connection pool");

        embedded_migrations::run(&conn_pool.get().unwrap()).expect("Failed to run migrations");

        Ok(Self { conn_pool })
    }

    pub fn get_channels(&self) -> Result<Vec<Channel>, diesel::result::Error> {
        let conn = self.conn_pool.get().unwrap();

        channels::table.order(channels::id).load(&conn)
    }

    pub fn get_channel(
        &self,
        channel_identifier: ChannelIdentifier,
    ) -> Result<Channel, diesel::result::Error> {
        let conn = self.conn_pool.get().unwrap();

        let query = channels::table.into_boxed();

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

                self.get_channel(channel_identifier.clone())
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
        channel_identifier: ChannelIdentifier,
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
        channel_identifier: ChannelIdentifier,
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

    pub fn get_user(&self, user_identifier: UserIdentifier) -> Result<User, diesel::result::Error> {
        let conn = self.conn_pool.get().unwrap();

        let query = users::table.into_boxed();

        let query = match &user_identifier {
            UserIdentifier::TwitchID(user_id) => {
                query.filter(users::twitch_id.eq_all(Some(user_id)))
            }
            UserIdentifier::DiscordID(user_id) => {
                query.filter(users::discord_id.eq_all(Some(user_id)))
            }
        };

        match query.first(&conn) {
            Ok(user) => Ok(user),
            Err(e) => match e {
                diesel::result::Error::NotFound => {
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

                    self.get_user(user_identifier.clone())
                }
                _ => Err(e),
            },
        }
    }
}
