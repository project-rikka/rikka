use std::sync::Arc;

const MAX_NAME: usize = 128;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityId(Arc<str>);

impl CapabilityId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_valid(&self) -> bool {
        let name = self.as_str();
        is_valid_member(name)
            && !name.bytes().any(|byte| byte.is_ascii_uppercase())
            && !name.starts_with('.')
            && !name.ends_with('.')
            && !name.contains("..")
    }
}

pub(crate) fn is_valid_member(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

impl From<&str> for CapabilityId {
    fn from(name: &str) -> Self {
        Self(Arc::from(name))
    }
}

impl From<String> for CapabilityId {
    fn from(name: String) -> Self {
        Self(Arc::from(name))
    }
}

impl From<&CapabilityId> for CapabilityId {
    fn from(id: &CapabilityId) -> Self {
        id.clone()
    }
}

impl std::fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::fmt::Debug for CapabilityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl serde::Serialize for CapabilityId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for CapabilityId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::from)
    }
}

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
}

impl Version {
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    pub const fn satisfies(self, required: Self) -> bool {
        self.major == required.major && self.minor >= required.minor
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}
