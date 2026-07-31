
#[derive(Debug, thiserror::Error)]
pub enum FcmError {
    #[error("missing config: {0}")]
    MissingConfig(String),

    #[error("auth: {0}")]
    Auth(String),

    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("api {0}: {1}")]
    Api(u16, String),

    #[error("token unregistered")]
    Unregistered,
}
