use thiserror::Error;

pub type Result<T> = std::result::Result<T, FtIndexError>;

#[derive(Debug, Error)]
pub enum FtIndexError {
    #[error("invalid option {key}: {message}")]
    InvalidOption { key: String, message: String },

    #[error("invalid query: {0}")]
    InvalidQuery(String),

    #[error("invalid storage format: {0}")]
    InvalidStorage(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("tantivy error: {0}")]
    Tantivy(#[from] tantivy::TantivyError),
}
