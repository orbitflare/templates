use thiserror::Error;

#[derive(Debug, Error)]
pub enum IndexerError {
    #[error("stream error: {0}")]
    Stream(String),

    #[error("connection failed: {0}")]
    Connection(String),

    #[error("database error: {0}")]
    Database(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("filter error: {0}")]
    Filter(String),

    #[error("api error: {0}")]
    Api(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, IndexerError>;
