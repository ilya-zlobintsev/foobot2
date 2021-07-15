use rocket::http::{ContentType, Status};
use rocket::response::Responder;
use rocket::response::{self};
use rocket::{Response, State};

use crate::database::models::WebSession;
use crate::database::DatabaseError;
use crate::{command_handler::CommandHandler};

#[post("/user/lastfm", data = "<name>")]
pub async fn set_lastfm_name(
    web_session: WebSession,
    cmd: &State<CommandHandler>,
    name: String,
) -> Status {
    cmd.db
        .set_lastfm_name(web_session.user_id, &name)
        .expect("DB Error");

    Status::Accepted
}

#[derive(Debug)]
pub enum ApiError {
    InvalidUser,
    DatabaseError(DatabaseError),
    RequestError(reqwest::Error),
    GenericError(String),
}

impl From<diesel::result::Error> for ApiError {
    fn from(e: diesel::result::Error) -> Self {
        Self::DatabaseError(DatabaseError::DieselError(e))
    }
}

impl From<DatabaseError> for ApiError {
    fn from(e: DatabaseError) -> Self {
        Self::DatabaseError(e)
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(e: reqwest::Error) -> Self {
        Self::RequestError(e)
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self::GenericError(e.to_string())
    }
}

impl<'a> Responder<'a, 'a> for ApiError {
    fn respond_to(self, _: &'a rocket::Request<'_>) -> response::Result<'static> {
        Response::build()
            .status(Status::NotFound)
            .header(ContentType::JSON)
            .ok()
    }
}
