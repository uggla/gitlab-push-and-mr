use crate::http;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    IO(std::io::Error),
    TomlParse(toml::de::Error),
    Git(String),
    Http(http::HttpError),
}

impl Error for AppError {
    fn description(&self) -> &str {
        match *self {
            AppError::IO(..) => "io error",
            AppError::TomlParse(..) => "toml parse error",
            AppError::Git(..) => "git error",
            AppError::Http(..) => "http error",
        }
    }
    fn cause(&self) -> Option<&dyn Error> {
        match *self {
            AppError::Git(..) => None,
            AppError::IO(ref e) => Some(e),
            AppError::TomlParse(ref e) => Some(e),
            AppError::Http(ref e) => Some(e),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            AppError::Git(ref v) => write!(f, "git error: {}", v),
            AppError::IO(ref e) => write!(f, "{}", e),
            AppError::TomlParse(ref e) => write!(f, "{}", e),
            AppError::Http(ref e) => write!(f, "{}", e),
        }
    }
}
impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::IO(e)
    }
}

impl From<toml::de::Error> for AppError {
    fn from(e: toml::de::Error) -> Self {
        AppError::TomlParse(e)
    }
}

impl From<http::HttpError> for AppError {
    fn from(e: http::HttpError) -> Self {
        AppError::Http(e)
    }
}
