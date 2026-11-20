use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Asset(Arc<str>);

impl Asset {
    #[must_use]
    pub fn new(path: &str) -> Self {
        Self(Arc::from(path))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for Asset {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Asset {
    fn from(path: &str) -> Self {
        Self::new(path)
    }
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}

impl Serialize for Asset {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Asset {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let path = std::borrow::Cow::<'de, str>::deserialize(deserializer)?;
        Ok(Self::new(&path))
    }
}
