use rikka_proton_pgp::SessionKey;

use crate::photo_properties::{PhotoInfo, PhotoProperties};

use crate::attributes::ExtendedAttributes;
use crate::error::{Error, Result};
use crate::identifier::{LinkId, RevisionId};
use crate::link::{ActiveRevision, FileProperties, Link};
use crate::node::NodeParts;
use crate::node_keys::NodeKeys;
use crate::signatures::Signatures;

#[derive(Debug, Clone)]
pub struct File {
    parts: NodeParts,
}

impl File {
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
    pub fn media_type(&self) -> &str {
        self.parts.link.media_type()
    }

    #[must_use]
    pub fn size(&self) -> i64 {
        self.parts.link.size()
    }

    #[must_use]
    pub fn active_revision(&self) -> Option<&ActiveRevision> {
        self.parts
            .link
            .file()
            .and_then(FileProperties::active_revision)
    }

    #[must_use]
    pub fn photo(&self) -> Option<&PhotoInfo> {
        self.active_revision().and_then(ActiveRevision::photo)
    }

    #[must_use]
    pub fn photo_properties(&self) -> Option<&PhotoProperties> {
        self.parts.link.photo()
    }

    #[must_use]
    pub fn is_proton_document(&self) -> bool {
        self.parts.link.is_proton_document()
    }

    #[must_use]
    pub fn sha1(&self) -> Option<&str> {
        self.attributes().and_then(ExtendedAttributes::sha1)
    }

    pub fn revision_id(&self) -> Result<&RevisionId> {
        self.active_revision()
            .map(ActiveRevision::id)
            .ok_or_else(|| Error::NoRevision(self.id().clone()))
    }

    pub fn content_key(&self) -> Result<&SessionKey> {
        self.parts
            .keys
            .content_key()
            .ok_or_else(|| Error::NotAFile(self.id().clone()))
    }

    pub(crate) fn new(parts: NodeParts) -> File {
        File { parts }
    }
}
