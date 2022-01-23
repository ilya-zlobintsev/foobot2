use chrono::{DateTime, Utc};
use twitch_irc::login::{TokenStorage, UserAccessToken};

use super::Database;

#[derive(Debug)]
pub struct Credentials {
    pub db: Database,
    pub user_id: String,
}

impl Credentials {
    fn make_entry_name(&self, entry: &str) -> String {
        format!("{}_{}", self.user_id, entry)
    }
}

#[async_trait]
impl TokenStorage for Credentials {
    type LoadError = anyhow::Error;
    type UpdateError = anyhow::Error;

    async fn load_token(&mut self) -> Result<UserAccessToken, Self::LoadError> {
        let access_token = self
            .db
            .get_auth(&self.make_entry_name("twitch_access_token"))?
            .unwrap_or_default();
        let refresh_token = self
            .db
            .get_auth(&self.make_entry_name("twitch_refresh_token"))?
            .unwrap_or_default();

        let created_at = DateTime::from_utc(
            DateTime::parse_from_rfc3339(
                &self
                    .db
                    .get_auth(&self.make_entry_name("twitch_created_at"))?
                    .unwrap_or_default(),
            )?
            .naive_utc(),
            Utc,
        );

        let expires_at = match self
            .db
            .get_auth(&self.make_entry_name("twitch_expires_at"))?
        {
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
        tracing::info!("Refreshed Twitch token for {}!", self.user_id);

        self.db.set_auth(
            &self.make_entry_name("twitch_access_token"),
            &token.access_token,
        )?;

        self.db.set_auth(
            &self.make_entry_name("twitch_refresh_token"),
            &token.refresh_token,
        )?;

        self.db.set_auth(
            &self.make_entry_name("twitch_created_at"),
            &token.created_at.to_rfc3339(),
        )?;

        if let Some(expires_at) = token.expires_at {
            self.db.set_auth(
                &self.make_entry_name("twitch_expires_at"),
                &expires_at.to_rfc3339(),
            )?;
        }

        Ok(())
    }
}
