use rikka_sync::Mutex;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

use rustc_hash::FxHashSet;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

static NAMES: Mutex<Option<FxHashSet<&'static str>>> = Mutex::new(None);

#[derive(Clone, Copy)]
pub struct Name(&'static str);

impl Name {
    #[must_use]
    pub fn new(text: &str) -> Self {
        let mut names = NAMES.lock();
        let names = names.get_or_insert_with(FxHashSet::default);
        if let Some(&known) = names.get(text) {
            return Self(known);
        }
        let leaked: &'static str = Box::leak(text.to_owned().into_boxed_str());
        names.insert(leaked);
        Self(leaked)
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        self.0
    }
}

impl Deref for Name {
    type Target = str;

    fn deref(&self) -> &str {
        self.0
    }
}

impl PartialEq for Name {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl Eq for Name {}

impl PartialEq<str> for Name {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for Name {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl Hash for Name {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialOrd for Name {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Name {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(other.0)
    }
}

impl From<&str> for Name {
    fn from(text: &str) -> Self {
        Self::new(text)
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.0, f)
    }
}

impl Serialize for Name {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0)
    }
}

impl<'de> Deserialize<'de> for Name {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = std::borrow::Cow::<'de, str>::deserialize(deserializer)?;
        Ok(Self::new(&text))
    }
}
