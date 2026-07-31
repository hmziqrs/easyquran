#[derive(Debug, thiserror::Error)]
pub enum FrameworkError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("provider API error: {0}")]
    ProviderApi(String),

    #[error("provider '{0}' is not registered")]
    ProviderNotRegistered(String),

    #[error("{0}")]
    Other(String),
}
