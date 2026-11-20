use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fingerprint(String);

impl Fingerprint {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<&str> for Fingerprint {
    fn from(value: &str) -> Self {
        Fingerprint(value.to_ascii_lowercase())
    }
}

impl From<String> for Fingerprint {
    fn from(value: String) -> Self {
        Fingerprint(value.to_ascii_lowercase())
    }
}

impl From<proton_rpgp::pgp::types::Fingerprint> for Fingerprint {
    fn from(value: proton_rpgp::pgp::types::Fingerprint) -> Self {
        Fingerprint(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyId(String);

impl KeyId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for KeyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<&str> for KeyId {
    fn from(value: &str) -> Self {
        KeyId(value.to_ascii_lowercase())
    }
}

impl From<String> for KeyId {
    fn from(value: String) -> Self {
        KeyId(value.to_ascii_lowercase())
    }
}

impl From<proton_rpgp::pgp::types::KeyId> for KeyId {
    fn from(value: proton_rpgp::pgp::types::KeyId) -> Self {
        KeyId(value.to_string())
    }
}
