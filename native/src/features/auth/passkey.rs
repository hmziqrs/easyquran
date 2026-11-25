//! The passkey flow (PARITY I5), `web/src/lib/auth/passkey-flow.svelte.ts`:
//! the server's challenge, the platform's sheet (`services::passkeys`), the
//! finish call. A sign-in ends in the machine every other one uses
//! (`AuthStore::complete_sign_in`), so a ban, a TOTP step and the rotated
//! session are handled the same way.

use eq_core::{
    api::{ApiError, SignInStep, TransitionKind, types::PasskeyView},
    i18n::keys,
    passkey::{CodecError, CreationOptions, RequestOptions},
};

use super::form::{self, Failure, Msg, copy::GENERIC_TRY_AGAIN};
use crate::{
    app::auth_store,
    services::passkeys::{self, PasskeyError},
};

/// Why a passkey flow did not finish.
pub(crate) enum PasskeyFailure {
    /// The user closed the sheet: nothing is shown (`flow.cancelled`).
    Cancelled,
    /// The device or the app cannot use passkeys (`PASSKEY_UNAVAILABLE`).
    Unavailable,
    /// The platform failed, or an answer was malformed.
    Broken,
    Api(ApiError),
}

impl From<PasskeyError> for PasskeyFailure {
    fn from(error: PasskeyError) -> Self {
        match error {
            PasskeyError::Cancelled => Self::Cancelled,
            PasskeyError::Unsupported => Self::Unavailable,
            PasskeyError::Failed => Self::Broken,
        }
    }
}

impl From<CodecError> for PasskeyFailure {
    /// The error names the field, never its value.
    fn from(error: CodecError) -> Self {
        log::warn!("passkeys: the server's options: {error}");
        Self::Broken
    }
}

impl From<ApiError> for PasskeyFailure {
    fn from(error: ApiError) -> Self {
        Self::Api(error)
    }
}

impl PasskeyFailure {
    /// The line shown for it (`genericError`); a cancellation shows none.
    pub(crate) fn message(&self) -> Option<Msg> {
        match self {
            Self::Cancelled => None,
            Self::Unavailable => Some(Msg::Key(keys::ACCOUNT_PASSKEYS_UNSUPPORTED)),
            Self::Broken => Some(Msg::Lit(GENERIC_TRY_AGAIN)),
            Self::Api(error) => match form::failure(error, &[]) {
                Failure::Message(message) => Some(message),
                Failure::Banned(_) | Failure::Fields(_) => Some(Msg::Lit(GENERIC_TRY_AGAIN)),
            },
        }
    }
}

/// `PasskeyFlow.login`.
pub(crate) async fn sign_in() -> Result<SignInStep, PasskeyFailure> {
    let store = auth_store();
    let client = store.client().ok_or(PasskeyFailure::Broken)?;
    let begin = match client.passkey_login_begin().await {
        Ok(begin) => begin,
        // Already signed in, or banned: the machine's to settle.
        Err(error) => {
            return Ok(store
                .complete_sign_in(Err(error), TransitionKind::Passkey)
                .await?);
        }
    };
    let options = RequestOptions::decode(&begin.challenge)?;
    let assertion = passkeys::assert(options).await?;
    let result = client
        .passkey_login_finish(assertion.credential(), &begin.state_handle)
        .await;
    Ok(store
        .complete_sign_in(result, TransitionKind::Passkey)
        .await?)
}

/// `PasskeyFlow.register`, without a label (the account page passes none).
pub(crate) async fn register() -> Result<PasskeyView, PasskeyFailure> {
    let client = auth_store().client().ok_or(PasskeyFailure::Broken)?;
    let begin = client.passkey_register_begin().await?;
    let options = CreationOptions::decode(&begin.challenge)?;
    let attestation = passkeys::register(options).await?;
    let made = client
        .passkey_register_finish(
            attestation.credential(),
            &begin.state_handle,
            None,
            attestation.transports(),
        )
        .await?;
    Ok(made)
}
