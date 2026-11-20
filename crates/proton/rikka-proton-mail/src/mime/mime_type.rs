use std::fmt;

use serde::de::{Deserialize, Deserializer};
use serde::ser::{Serialize, Serializer};

pub const TEXT_HTML: &str = "text/html";
pub const TEXT_PLAIN: &str = "text/plain";
pub const MULTIPART_MIXED: &str = "multipart/mixed";

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MimeType {
    #[default]
    Html,
    PlainText,
    MultipartMixed,
    Other(String),
}

impl MimeType {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            MimeType::Html => TEXT_HTML,
            MimeType::PlainText => TEXT_PLAIN,
            MimeType::MultipartMixed => MULTIPART_MIXED,
            MimeType::Other(value) => value,
        }
    }

    #[must_use]
    pub fn is_mime(&self) -> bool {
        matches!(self, MimeType::MultipartMixed)
    }
}

impl From<&str> for MimeType {
    fn from(value: &str) -> MimeType {
        match value.trim() {
            TEXT_HTML => MimeType::Html,
            TEXT_PLAIN => MimeType::PlainText,
            MULTIPART_MIXED => MimeType::MultipartMixed,
            other => MimeType::Other(other.to_owned()),
        }
    }
}

impl From<String> for MimeType {
    fn from(value: String) -> MimeType {
        MimeType::from(value.as_str())
    }
}

impl fmt::Display for MimeType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for MimeType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for MimeType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<MimeType, D::Error> {
        Ok(MimeType::from(String::deserialize(deserializer)?))
    }
}
