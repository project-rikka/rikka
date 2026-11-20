use std::collections::BTreeMap;

use zeroize::Zeroizing;

pub type Attributes = BTreeMap<String, String>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub(crate) id: u64,
    pub(crate) label: String,
    pub(crate) attributes: Attributes,
    pub(crate) content_type: String,
    pub(crate) created: u64,
    pub(crate) modified: u64,
}

impl Item {
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    #[must_use]
    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    #[must_use]
    pub fn created(&self) -> u64 {
        self.created
    }

    #[must_use]
    pub fn modified(&self) -> u64 {
        self.modified
    }

    #[must_use]
    pub fn matches(&self, query: &Attributes) -> bool {
        query
            .iter()
            .all(|(name, value)| self.attributes.get(name) == Some(value))
    }
}

pub struct NewItem<'a> {
    pub label: &'a str,
    pub attributes: Attributes,
    pub content_type: &'a str,
    pub secret: &'a [u8],
}

impl std::fmt::Debug for NewItem<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NewItem")
            .field("label", &self.label)
            .field("attributes", &self.attributes)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stored {
    pub id: u64,
    pub replaced: bool,
}

pub struct Secret {
    pub value: Zeroizing<Vec<u8>>,
    pub content_type: String,
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Secret")
            .field("content_type", &self.content_type)
            .finish_non_exhaustive()
    }
}
