use crate::attributes::ExtendedAttributes;
use crate::error::{Error, Result};
use crate::hash_key::HashKey;
use crate::identifier::LinkId;
use crate::link::Link;
use crate::node::NodeParts;
use crate::node_keys::NodeKeys;
use crate::photo_properties::AlbumProperties;
use crate::signatures::Signatures;

#[derive(Debug, Clone)]
pub struct Folder {
    parts: NodeParts,
}

impl Folder {
    #[must_use]
    pub fn id(&self) -> &LinkId {
        self.parts.link.id()
    }

    #[must_use]
    pub fn parent_id(&self) -> Option<&LinkId> {
        self.parts.link.parent_id()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.parts.name
    }

    #[must_use]
    pub fn link(&self) -> &Link {
        &self.parts.link
    }

    #[must_use]
    pub fn keys(&self) -> &NodeKeys {
        &self.parts.keys
    }

    #[must_use]
    pub fn signatures(&self) -> &Signatures {
        &self.parts.signatures
    }

    #[must_use]
    pub fn attributes(&self) -> Option<&ExtendedAttributes> {
        self.parts.attributes.as_ref()
    }

    #[must_use]
    pub fn is_album(&self) -> bool {
        self.parts.link.is_album()
    }

    #[must_use]
    pub fn album(&self) -> Option<&AlbumProperties> {
        self.parts.link.album()
    }

    pub fn hash_key(&self) -> Result<&HashKey> {
        self.parts
            .keys
            .hash_key()
            .ok_or_else(|| Error::NoHashKey(self.id().clone()))
    }

    pub fn hash(&self, name: &str) -> Result<String> {
        Ok(self.hash_key()?.hash(name))
    }

    pub(crate) fn new(parts: NodeParts) -> Folder {
        Folder { parts }
    }
}
