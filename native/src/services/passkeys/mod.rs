//! Passkeys (PARITY I5): the platform authenticator behind one async
//! interface. NSH has no passkey API (`nsh-biometrics` is local auth only),
//! so the backends call the platform themselves:
//!
//! - iOS 16, macOS 13 (`apple`): `ASAuthorizationController` with
//!   `ASAuthorizationPlatformPublicKeyCredentialProvider`. The app must be
//!   signed with `webcredentials:<rp id>` in its associated domains, and the
//!   site must serve the app's id. `nsh.json` `ios.entitlements` holds it for
//!   iOS. A macOS app with it runs only under a provisioning profile, so it
//!   is left out there and [`available`] is false until a build adds it.
//! - Android 14 (`android`): the framework's `android.credentials`, reached
//!   over JNI. Older versions need Google's Credential Manager libraries,
//!   which the app does not ship.
//! - Windows and Linux: not offered (`contracts/devices.md` "Sign-in
//!   matrix": desktop, not at launch; Linux has no system authenticator).
//!
//! Nothing here logs a challenge, a credential id or an account: failures
//! are logged by their platform error code or type only.

#[cfg(target_os = "android")]
mod android;
#[cfg(any(target_os = "ios", target_os = "macos"))]
mod apple;

#[cfg(target_os = "android")]
use android as platform;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use apple as platform;
#[cfg(not(any(target_os = "android", target_os = "ios", target_os = "macos")))]
use unsupported as platform;

use eq_core::passkey::{Assertion, Attestation, CreationOptions, RequestOptions};

/// Why the authenticator gave no answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasskeyError {
    /// The user closed the system's sheet.
    Cancelled,
    /// This device or request has no passkey support.
    Unsupported,
    /// The platform refused or failed (its code is in the log).
    Failed,
}

/// Whether this device can use passkeys for the app; the UI hides them when
/// it cannot (the web's `passkey.supported`).
pub async fn available() -> bool {
    platform::available()
}

/// Makes a passkey, over the system's sheet.
pub async fn register(options: CreationOptions) -> Result<Attestation, PasskeyError> {
    if !platform::available() {
        return Err(PasskeyError::Unsupported);
    }
    platform::register(options).await
}

/// Signs a sign-in with a passkey, over the system's sheet.
pub async fn assert(options: RequestOptions) -> Result<Assertion, PasskeyError> {
    if !platform::available() {
        return Err(PasskeyError::Unsupported);
    }
    platform::assert(options).await
}

#[cfg(not(any(target_os = "android", target_os = "ios", target_os = "macos")))]
mod unsupported {
    use super::*;

    pub fn available() -> bool {
        false
    }

    pub async fn register(_: CreationOptions) -> Result<Attestation, PasskeyError> {
        Err(PasskeyError::Unsupported)
    }

    pub async fn assert(_: RequestOptions) -> Result<Assertion, PasskeyError> {
        Err(PasskeyError::Unsupported)
    }
}
