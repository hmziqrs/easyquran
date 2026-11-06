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

    /// 400 INVALID_ARGUMENT. Usually a bad token, but a bad payload gives the same
    /// status, so callers prune only after repeated failures.
    #[error("invalid argument")]
    InvalidArgument,

    /// 403 SENDER_ID_MISMATCH: the token belongs to another Firebase sender.
    #[error("sender id mismatch")]
    SenderMismatch,
}
