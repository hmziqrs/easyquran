//! [`Secret`]: a string that never prints. The session value, the CSRF
//! token, passwords, one-time codes, TOTP and reset tokens, 2FA secrets and
//! the device private key travel in it, so a stray `{:?}` in a log, a trace
//! or an error report shows `Secret(***)` and nothing else.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A secret string. `Debug` is redacted; there is no `Display`. Read the
/// value with [`Secret::expose`] only where it goes on the wire or into the
/// secret store.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Secret(String);

impl Secret {
    /// Wraps `value`.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The value. Never log it.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Whether the value is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Secret {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

impl Serialize for Secret {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_shows_the_value() {
        let s = Secret::new("hunter2-session");
        assert_eq!(format!("{s:?}"), "Secret(***)");
        assert!(!format!("{:#?}", Some(&s)).contains("hunter2"));
    }

    #[test]
    fn serde_is_the_plain_string() {
        let s = Secret::new("abc");
        assert_eq!(serde_json::to_string(&s).unwrap(), "\"abc\"");
        let back: Secret = serde_json::from_str("\"abc\"").unwrap();
        assert_eq!(back.expose(), "abc");
    }
}
