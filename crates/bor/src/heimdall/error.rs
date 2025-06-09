use thiserror::Error;

#[derive(Error, Debug)]
pub enum HeimdallError {
    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("URL parsing error: {0}")]
    UrlError(#[from] url::ParseError),

    #[error("JSON deserialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Heimdall returned an unsuccessful status code: {0}")]
    UnsuccessfulResponse(reqwest::StatusCode),

    #[error("Heimdall returned no response body")]
    NoResponse,

    #[error("Pack error")]
    PackError(),

    #[error("Unpack error")]
    UnpackError,

    #[error("System time error: {0}")]
    SystemTimeError(#[from] std::time::SystemTimeError),

    #[error("Sol decode error: {0}")]
    SolDecodeError(String),

    #[error("EVM error")]
    EVMError,

    #[error("Invalid state sync data")]
    InvalidStateSyncData,
}
