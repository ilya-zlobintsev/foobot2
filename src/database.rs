mod models;
mod schema;

use diesel::{ConnectionError, EqAll, QueryDsl, RunQueryDsl, mysql::MysqlConnection, r2d2::{self, ConnectionManager, Pool}};

use crate::{database::schema::users::*, platform::UserIdentifier};
use crate::database::schema::users::dsl::users;
use self::models::User;

#[derive(Clone)]
pub struct Database {
    conn_pool: Pool<ConnectionManager<MysqlConnection>>,
}

impl Database {
    pub fn connect(database_url: String) -> Result<Self, ConnectionError> {
        let manager = ConnectionManager::<MysqlConnection>::new(database_url);
        let conn_pool = r2d2::Pool::new(manager).expect("Failed to set up DB connection pool");
        
        diesel_migrations::run_pending_migrations(&conn_pool.get().unwrap()).expect("Failed to run DB migrations");

        Ok(Self { conn_pool })
    }

    pub fn get_user(
        &self,
        user_identifier: UserIdentifier
    ) -> Result<Option<User>, diesel::result::Error> {
        let conn = self.conn_pool.get().unwrap();

        let query = users.into_boxed();

        let query = match user_identifier {
            UserIdentifier::TwitchID(user_id) => query.filter(twitch_id.eq_all(Some(user_id))),
            UserIdentifier::DiscordID(user_id) => query.filter(discord_id.eq_all(Some(user_id))),
        };

        match query.first(&conn) {
            Ok(user) => Ok(Some(user)),
            Err(e) => match e {
                diesel::result::Error::NotFound => Ok(None),
                _ => Err(e)
            }
        }
    }
}