// Fail-closed: callers MUST deny on any GateError (store can't vouch for the count).
#[derive(Debug, thiserror::Error)]
pub enum GateError {
    #[error("rate-limit store unavailable: {0}")]
    StoreUnavailable(String),
    #[error("rate-limit store returned an unexpected result")]
    UnexpectedResult,
}
