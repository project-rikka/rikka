use jiff::Timestamp;
use rikka_proton_api::wire_enum;
use rikka_proton_pgp::SignatureStatus;
use serde::Deserialize;

use crate::attributes::ExtendedAttributes;
use crate::identifier::RevisionId;
use crate::thumbnail::ThumbnailInfo;

wire_enum! {
    pub enum RevisionState: default Active {
        Draft = 0,
        Active = 1,
        Obsolete = 2,
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Block {
    #[serde(rename = "Index")]
    index: u32,
    #[serde(rename = "BareURL", default)]
    bare_url: Option<String>,
    #[serde(rename = "Token", default)]
    token: Option<String>,
    #[serde(rename = "URL", default)]
    url: Option<String>,
    #[serde(rename = "Hash", default)]
    hash: Option<String>,
    #[serde(rename = "EncSignature", default)]
    encrypted_signature: Option<String>,
    #[serde(rename = "SignatureEmail", default)]
    signature_email: Option<String>,
}

impl Block {
    #[must_use]
    pub fn index(&self) -> u32 {
        self.index
    }

    #[must_use]
    pub fn bare_url(&self) -> Option<&str> {
        self.bare_url.as_deref()
    }

    #[must_use]
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    #[must_use]
    pub fn hash(&self) -> Option<&str> {
        self.hash.as_deref()
    }

    #[must_use]
    pub fn encrypted_signature(&self) -> Option<&str> {
        self.encrypted_signature.as_deref()
    }

    #[must_use]
    pub fn signature_email(&self) -> Option<&str> {
        self.signature_email.as_deref()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RevisionInfo {
    #[serde(rename = "ID")]
    id: RevisionId,
    #[serde(rename = "State", default)]
    state: RevisionState,
    #[serde(rename = "Size", default)]
    size: i64,
    #[serde(rename = "ManifestSignature", default)]
    manifest_signature: Option<String>,
    #[serde(rename = "SignatureAddress", default)]
    signature_address: Option<String>,
    #[serde(rename = "SignatureEmail", default)]
    signature_email: Option<String>,
    #[serde(rename = "XAttr", default)]
    extended_attributes: Option<String>,
    #[serde(rename = "ChecksumVerified", default)]
    checksum_verified: bool,
    #[serde(rename = "Blocks", default)]
    blocks: Vec<Block>,
    #[serde(rename = "Thumbnails", default)]
    thumbnails: Vec<ThumbnailInfo>,
    #[serde(
        rename = "CreateTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    created_at: Option<Timestamp>,
}

impl RevisionInfo {
    #[must_use]
    pub fn id(&self) -> &RevisionId {
        &self.id
    }

    #[must_use]
    pub fn state(&self) -> RevisionState {
        self.state
    }

    #[must_use]
    pub fn size(&self) -> i64 {
        self.size
    }

    #[must_use]
    pub fn manifest_signature(&self) -> Option<&str> {
        self.manifest_signature.as_deref()
    }

    #[must_use]
    pub fn signature_email(&self) -> Option<&str> {
        crate::link::signer(
            self.signature_email.as_deref(),
            self.signature_address.as_deref(),
        )
    }

    #[must_use]
    pub fn extended_attributes(&self) -> Option<&str> {
        self.extended_attributes.as_deref()
    }

    #[must_use]
    pub fn is_checksum_verified(&self) -> bool {
        self.checksum_verified
    }

    #[must_use]
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    #[must_use]
    pub fn thumbnails(&self) -> &[ThumbnailInfo] {
        &self.thumbnails
    }

    #[must_use]
    pub fn created_at(&self) -> Option<Timestamp> {
        self.created_at
    }

    pub(crate) fn drain_blocks(&mut self) -> Vec<Block> {
        std::mem::take(&mut self.blocks)
    }

    pub(crate) fn with_blocks(mut self, blocks: Vec<Block>) -> RevisionInfo {
        self.blocks = blocks;
        self
    }
}

#[derive(Debug, Clone)]
pub struct Revision {
    info: RevisionInfo,
    attributes: Option<ExtendedAttributes>,
    signature: SignatureStatus,
}

impl Revision {
    #[must_use]
    pub fn id(&self) -> &RevisionId {
        self.info.id()
    }

    #[must_use]
    pub fn info(&self) -> &RevisionInfo {
        &self.info
    }

    #[must_use]
    pub fn state(&self) -> RevisionState {
        self.info.state()
    }

    #[must_use]
    pub fn size(&self) -> i64 {
        self.info.size()
    }

    #[must_use]
    pub fn blocks(&self) -> &[Block] {
        self.info.blocks()
    }

    #[must_use]
    pub fn thumbnails(&self) -> &[ThumbnailInfo] {
        self.info.thumbnails()
    }

    #[must_use]
    pub fn created_at(&self) -> Option<Timestamp> {
        self.info.created_at()
    }

    #[must_use]
    pub fn attributes(&self) -> Option<&ExtendedAttributes> {
        self.attributes.as_ref()
    }

    #[must_use]
    pub fn signature(&self) -> &SignatureStatus {
        &self.signature
    }

    pub(crate) fn new(
        info: RevisionInfo,
        attributes: Option<ExtendedAttributes>,
        signature: SignatureStatus,
    ) -> Revision {
        Revision {
            info,
            attributes,
            signature,
        }
    }
}
