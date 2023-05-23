use crate::{database::DatabaseError, platform::UserIdentifierError};
use std::{env::VarError, fmt, num::ParseIntError};

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    MissingArgument(String),
    InvalidArgument(String),
    NoPermissions,
    DatabaseError(#[from] DatabaseError),
    TemplateError(#[from] handlebars::RenderError),
    ConfigurationError(#[from] VarError),
    GenericError(String),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::MissingArgument(arg) => {
                f.write_str(&format!("missing argument: {}", arg))
            }
            CommandError::InvalidArgument(arg) => {
                f.write_str(&format!("invalid argument: {}", arg))
            }
            CommandError::NoPermissions => {
                f.write_str("you don't have the permissions to use this command")
            }
            CommandError::DatabaseError(e) => f.write_str(&e.to_string()),
            CommandError::TemplateError(e) => f.write_str(&e.to_string()),
            CommandError::ConfigurationError(e) => {
                f.write_str(&format!("configuration error: {}", e))
            }
            CommandError::GenericError(s) => f.write_str(s),
        }
    }
}

impl From<diesel::result::Error> for CommandError {
    fn from(e: diesel::result::Error) -> Self {
        Self::DatabaseError(DatabaseError::DieselError(e))
    }
}

impl From<ParseIntError> for CommandError {
    fn from(_: ParseIntError) -> Self {
        Self::InvalidArgument("expected a number".to_string())
    }
}

impl From<UserIdentifierError> for CommandError {
    fn from(e: UserIdentifierError) -> Self {
        match e {
            UserIdentifierError::MissingDelimiter => Self::MissingArgument(
                "separator `:`! Must be in the form of `platform:user`".to_string(),
            ),
            UserIdentifierError::InvalidPlatform => Self::InvalidArgument("platform".to_string()),
            UserIdentifierError::InvalidId => Self::InvalidArgument("invalid user id".to_owned()),
        }
    }
}

impl From<anyhow::Error> for CommandError {
    fn from(e: anyhow::Error) -> Self {
        Self::GenericError(e.to_string())
    }
}

impl From<&'static str> for CommandError {
    fn from(msg: &'static str) -> Self {
        CommandError::GenericError(msg.to_owned())
    }
}
