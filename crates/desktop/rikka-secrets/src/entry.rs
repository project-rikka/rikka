use std::collections::BTreeMap;

use rikka_secrets_protocol::names::SCHEMA_ATTRIBUTE;
#[cfg(unix)]
use zbus::zvariant::OwnedObjectPath;
use zeroize::Zeroizing;

pub type Attributes = BTreeMap<String, String>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Entry {
    attributes: Attributes,
}

impl Entry {
    #[must_use]
    pub fn new(schema: impl Into<String>) -> Entry {
        Entry {
            attributes: BTreeMap::from([(SCHEMA_ATTRIBUTE.to_owned(), schema.into())]),
        }
    }

    #[must_use]
    pub fn from_attributes(attributes: Attributes) -> Entry {
        Entry { attributes }
    }

    #[must_use]
    pub fn attribute(mut self, name: impl Into<String>, value: impl Into<String>) -> Entry {
        self.attributes.insert(name.into(), value.into());
        self
    }

    #[must_use]
    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }
}

pub struct Secret(Zeroizing<Vec<u8>>);

impl Secret {
    pub(crate) fn new(bytes: Zeroizing<Vec<u8>>) -> Secret {
        Secret(bytes)
    }

    #[must_use]
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Secret").finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct Found {
    pub label: String,
    pub attributes: Attributes,
    pub secret: Secret,
}

#[cfg(unix)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ItemId(pub(crate) OwnedObjectPath);

#[cfg(windows)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ItemId(pub(crate) String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredItem {
    pub id: ItemId,
    pub label: String,
    pub attributes: Attributes,
    pub created: u64,
    pub modified: u64,
}

impl StoredItem {
    #[must_use]
    pub fn schema(&self) -> Option<&str> {
        self.attributes.get(SCHEMA_ATTRIBUTE).map(String::as_str)
    }
}
