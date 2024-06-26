use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("JSON parsing error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("UTF-8 error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("No configuration received")]
    NoConfigReceived,

    #[error("Configuration error: {0}")]
    GenericError(String), // Generic error variant

}
