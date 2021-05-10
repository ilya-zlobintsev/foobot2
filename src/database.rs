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

#[derive(Clone)]
pub struct Database {
    conn_pool: Pool<ConnectionManager<MysqlConnection>>,
}

impl Database {
    pub fn connect(database_url: String) -> Result<Self, ConnectionError> {
        let manager = ConnectionManager::<MysqlConnection>::new(database_url);
        let conn_pool = r2d2::Pool::new(manager).expect("Failed to set up DB connection pool");

        diesel_migrations::run_pending_migrations(&conn_pool.get().unwrap())
            .expect("Failed to run DB migrations");

        Ok(Self { conn_pool })
    }

    pub fn get_channels(&self) -> Vec<Channel> {
        let conn = self.conn_pool.get().unwrap();

        channels::table.load(&conn).expect("Failed to get channels")
    }

    pub fn get_channel(
        &self,
        channel_identifier: ChannelIdentifier,
    ) -> Result<Channel, diesel::result::Error> {
        let conn = self.conn_pool.get().unwrap();

        let query = channels::table.into_boxed();

        let query = match &channel_identifier {
            ChannelIdentifier::TwitchChannelName(channel_name) => query
                .filter(channels::platform.eq_all(channel_identifier.get_platform_name()))
                .filter(channels::channel.eq_all(channel_name)),
            ChannelIdentifier::DiscordGuildID(guild_id) => query
                .filter(channels::platform.eq_all(channel_identifier.get_platform_name()))
                .filter(channels::channel.eq_all(guild_id)),
            ChannelIdentifier::DiscordChannelID(ch_id) => query
                .filter(channels::platform.eq_all(channel_identifier.get_platform_name()))
                .filter(channels::channel.eq_all(ch_id)),
        };

        match query.load(&conn)?.into_iter().next() {
            Some(channel) => Ok(channel),
            None => {
                let new_channel = NewChannel {
                    platform: channel_identifier.get_platform_name(),
                    channel: channel_identifier.get_channel(),
                };

                diesel::insert_into(channels::table)
                    .values(new_channel)
                    .execute(&conn)
                    .expect("Failed to create channel");

                self.get_channel(channel_identifier.clone())
            }
        }
    }

    pub fn get_command(
        &self,
        channel_identifier: ChannelIdentifier,
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

    pub fn add_command(
        &self,
        channel_identifier: ChannelIdentifier,
        command_name: &str,
        command_action: &str,
    ) -> Result<(), diesel::result::Error> {
        let conn = self.conn_pool.get().unwrap();

        diesel::insert_into(commands::table)
            .values(
                &NewCommand {
                    name: command_name,
                    action: command_action,
                    permissions: None,
                    channel_id:
                        channels::table // I couldn't figure out how to do this as a subquery
                            .filter(
                                channels::platform.eq_all(channel_identifier.get_platform_name()),
                            )
                            .filter(channels::channel.eq_all(channel_identifier.get_channel()))
                            .select(channels::id)
                            .first(&conn)
                            .unwrap(),
                },
            )
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
                .filter(commands::name.eq_all(command_name))
        ).execute(&conn)?;

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
