#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Version {
    V2_1,
    V3,
    #[default]
    V4,
    Other(String),
}

impl Version {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Version::V2_1 => "2.1",
            Version::V3 => "3.0",
            Version::V4 => "4.0",
            Version::Other(other) => other,
        }
    }
}

impl From<&str> for Version {
    fn from(value: &str) -> Version {
        match value.trim() {
            "2.1" => Version::V2_1,
            "3.0" | "3" => Version::V3,
            "4.0" | "4" => Version::V4,
            other => Version::Other(other.to_owned()),
        }
    }
}
