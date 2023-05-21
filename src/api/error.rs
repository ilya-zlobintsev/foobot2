use crate::{command_handler::error::CommandError, database::DatabaseError};
use axum::response::IntoResponse;
use http::StatusCode;

#[derive(Debug)]
pub enum ApiError {
    NotFound,
    BadRequest(String),
    InvalidUser,
    Unauthorized(String),
    DatabaseError(DatabaseError),
    RequestError(reqwest::Error),
    CommandError(CommandError),
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

impl From<CommandError> for ApiError {
    fn from(value: CommandError) -> Self {
        Self::CommandError(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        tracing::info!("Responding with error {self:?}");
        match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
            ApiError::NotFound | ApiError::InvalidUser => StatusCode::NOT_FOUND.into_response(),
            ApiError::CommandError(err) => {
                (StatusCode::UNPROCESSABLE_ENTITY, err.to_string()).into_response()
            }
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
}
