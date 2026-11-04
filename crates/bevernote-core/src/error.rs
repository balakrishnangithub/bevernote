use thiserror::Error;

#[derive(Debug, Error)]
pub enum BeverError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("invalid data: {0}")]
    Invalid(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("remote sync error: {0}")]
    Remote(String),
}

impl From<serde_json::Error> for BeverError {
    fn from(value: serde_json::Error) -> Self {
        Self::Invalid(value.to_string())
    }
}

impl From<reqwest::Error> for BeverError {
    fn from(value: reqwest::Error) -> Self {
        Self::Remote(value.to_string())
    }
}

pub type Result<T> = std::result::Result<T, BeverError>;
