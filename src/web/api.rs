use std::io::Cursor;

use rocket::http::Status;
use rocket::response::Responder;
use rocket::response::{self};
use rocket::{Response, State};

use crate::command_handler::CommandHandler;
use crate::database::models::WebSession;
use crate::database::DatabaseError;

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
    NotFound,
    BadRequest(String),
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
        let mut response = Response::build();

        match self {
            Self::BadRequest(msg) => response
                .status(Status::BadRequest)
                .sized_body(msg.len(), Cursor::new(msg)),
            ApiError::NotFound => response.status(Status::NotFound),
            ApiError::InvalidUser => response.status(Status::NotFound),
            _ => response.status(Status::InternalServerError),
        };

        response.ok()
    }
}
