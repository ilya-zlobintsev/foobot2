use crate::{command_handler::error::CommandError, database::DatabaseError};
use rocket::{
    http::Status,
    response::{self, Responder},
    Response,
};
use rocket_okapi::{
    gen::OpenApiGenerator,
    okapi::{openapi3::Responses, schemars::Map},
    response::OpenApiResponderInner,
    OpenApiError,
};
use std::io::Cursor;

#[derive(Debug)]
pub enum ApiError {
    NotFound,
    BadRequest(String),
    InvalidUser,
    Unauthorized(String),
    DatabaseError(DatabaseError),
    RequestError(reqwest::Error),
    CommandErorr(CommandError),
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
        Self::CommandErorr(value)
    }
}

impl<'a> Responder<'a, 'a> for ApiError {
    fn respond_to(self, _: &'a rocket::Request<'_>) -> response::Result<'static> {
        let mut response = Response::build();

        tracing::info!("Responding with error {self:?}");
        match self {
            Self::BadRequest(msg) => response
                .status(Status::BadRequest)
                .sized_body(msg.len(), Cursor::new(msg)),
            ApiError::NotFound => response.status(Status::NotFound),
            ApiError::InvalidUser => response.status(Status::NotFound),
            ApiError::CommandErorr(err) => {
                let error_text = err.to_string();
                response
                    .status(Status::UnprocessableEntity)
                    .sized_body(error_text.len(), Cursor::new(error_text))
            }
            ApiError::Unauthorized(msg) => response
                .status(Status::Unauthorized)
                .sized_body(msg.len(), Cursor::new(msg)),
            _ => response.status(Status::InternalServerError),
        };

        response.ok()
    }
}

impl OpenApiResponderInner for ApiError {
    fn responses(_generator: &mut OpenApiGenerator) -> Result<Responses, OpenApiError> {
        use rocket_okapi::okapi::openapi3::{RefOr, Response as OpenApiReponse};

        let mut responses = Map::new();
        responses.insert(
            "400".to_string(),
            RefOr::Object(OpenApiReponse {
                description: "\
                # [400 Bad Request](https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/400)\n\
                The request given is wrongly formatted or data asked could not be fulfilled. \
                "
                .to_string(),
                ..Default::default()
            }),
        );
        responses.insert(
            "404".to_string(),
            RefOr::Object(OpenApiReponse {
                description: "\
                # [404 Not Found](https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/404)\n\
                This response is given when you request a page that does not exists.\
                "
                .to_string(),
                ..Default::default()
            }),
        );
        responses.insert(
            "422".to_string(),
            RefOr::Object(OpenApiReponse {
                description: "\
                # [422 Unprocessable Entity](https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/422)\n\
                This response is given when you request body is not correctly formatted. \
                ".to_string(),
                ..Default::default()
            }),
        );
        responses.insert(
            "500".to_string(),
            RefOr::Object(OpenApiReponse {
                description: "\
                # [500 Internal Server Error](https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/500)\n\
                This response is given when something wend wrong on the server. \
                ".to_string(),
                ..Default::default()
            }),
        );
        Ok(Responses {
            responses,
            ..Default::default()
        })
    }
}
