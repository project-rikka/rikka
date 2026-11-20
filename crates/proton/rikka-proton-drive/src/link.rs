use jiff::Timestamp;
use rikka_proton_api::wire_enum;
use serde::Deserialize;

use crate::identifier::{LinkId, RevisionId, VolumeId};
use crate::link_wire::LinkDetails;
use crate::photo_properties::{AlbumProperties, PhotoInfo, PhotoProperties};
use crate::revision::RevisionState;
use crate::role::Role;
use crate::sharing_details::SharingDetails;
use crate::thumbnail::{ThumbnailInfo, ThumbnailType};

pub const PROTON_DOCUMENT_MEDIA_TYPE: &str = "application/vnd.proton.doc";
pub const PROTON_SHEET_MEDIA_TYPE: &str = "application/vnd.proton.sheet";

wire_enum! {
    pub enum LinkType: default Folder {
        Folder = 1,
        File = 2,
        Album = 3,
    }
}

wire_enum! {
    pub enum LinkState: default Active {
        Draft = 0,
        Active = 1,
        Trashed = 2,
    }
}

pub(crate) fn signer<'a>(email: Option<&'a str>, address: Option<&'a str>) -> Option<&'a str> {
    let named = |signer: &&'a str| !signer.is_empty();

    email.filter(named).or(address.filter(named))
}

#[derive(Debug, Clone)]
pub struct FolderProperties {
    pub(crate) node_hash_key: Option<String>,
}

impl FolderProperties {
    #[must_use]
    pub fn node_hash_key(&self) -> Option<&str> {
        self.node_hash_key.as_deref()
    }
}

#[derive(Debug, Clone)]
pub struct FileProperties {
    pub(crate) content_key_packet: Option<String>,
    pub(crate) content_key_packet_signature: Option<String>,
    pub(crate) active_revision: Option<ActiveRevision>,
}

impl FileProperties {
    #[must_use]
    pub fn content_key_packet(&self) -> Option<&str> {
        self.content_key_packet.as_deref()
    }

    #[must_use]
    pub fn content_key_packet_signature(&self) -> Option<&str> {
        self.content_key_packet_signature.as_deref()
    }

    #[must_use]
    pub fn active_revision(&self) -> Option<&ActiveRevision> {
        self.active_revision.as_ref()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ActiveRevision {
    #[serde(rename = "RevisionID")]
    id: RevisionId,
    #[serde(rename = "EncryptedSize", default)]
    size: i64,
    #[serde(rename = "ManifestSignature", default)]
    manifest_signature: Option<String>,
    #[serde(rename = "SignatureEmail", default)]
    signature_email: Option<String>,
    #[serde(rename = "XAttr", default)]
    extended_attributes: Option<String>,
    #[serde(rename = "Thumbnails", default)]
    thumbnails: Vec<ThumbnailInfo>,
    #[serde(
        rename = "CreateTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    created_at: Option<Timestamp>,
    #[serde(rename = "ChecksumVerified", default)]
    checksum_verified: bool,
    #[serde(rename = "Photo", default)]
    photo: Option<PhotoInfo>,
}

impl ActiveRevision {
    #[must_use]
    pub fn id(&self) -> &RevisionId {
        &self.id
    }

    #[must_use]
    pub fn size(&self) -> i64 {
        self.size
    }

    #[must_use]
    pub fn state(&self) -> RevisionState {
        RevisionState::Active
    }

    #[must_use]
    pub fn manifest_signature(&self) -> Option<&str> {
        self.manifest_signature.as_deref()
    }

    #[must_use]
    pub fn signature_email(&self) -> Option<&str> {
        signer(self.signature_email.as_deref(), None)
    }

    #[must_use]
    pub fn extended_attributes(&self) -> Option<&str> {
        self.extended_attributes.as_deref()
    }

    #[must_use]
    pub fn created_at(&self) -> Option<Timestamp> {
        self.created_at
    }

    #[must_use]
    pub fn thumbnails(&self) -> &[ThumbnailInfo] {
        &self.thumbnails
    }

    #[must_use]
    pub fn has_thumbnail(&self, kind: ThumbnailType) -> bool {
        self.thumbnail(kind).is_some()
    }

    #[must_use]
    pub fn thumbnail(&self, kind: ThumbnailType) -> Option<&ThumbnailInfo> {
        self.thumbnails
            .iter()
            .find(|candidate| candidate.kind() == kind)
    }

    #[must_use]
    pub fn is_checksum_verified(&self) -> bool {
        self.checksum_verified
    }

    #[must_use]
    pub fn photo(&self) -> Option<&PhotoInfo> {
        self.photo.as_ref()
    }

    pub(crate) fn with_photo(mut self, photo: Option<PhotoInfo>) -> ActiveRevision {
        if self.photo.is_none() {
            self.photo = photo;
        }
        self
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(from = "LinkDetails")]
pub struct Link {
    pub(crate) id: LinkId,
    pub(crate) parent_id: Option<LinkId>,
    pub(crate) volume_id: Option<VolumeId>,
    pub(crate) kind: LinkType,
    pub(crate) state: LinkState,
    pub(crate) name: String,
    pub(crate) hash: Option<String>,
    pub(crate) media_type: String,
    pub(crate) size: i64,
    pub(crate) node_key: String,
    pub(crate) node_passphrase: String,
    pub(crate) node_passphrase_signature: Option<String>,
    pub(crate) signature_email: Option<String>,
    pub(crate) name_signature_email: Option<String>,
    pub(crate) extended_attributes: Option<String>,
    pub(crate) file: Option<FileProperties>,
    pub(crate) folder: Option<FolderProperties>,
    pub(crate) album: Option<AlbumProperties>,
    pub(crate) photo: Option<PhotoProperties>,
    pub(crate) sharing: Option<SharingDetails>,
    pub(crate) permissions: Option<Role>,
    pub(crate) owner: Option<String>,
    pub(crate) created_at: Option<Timestamp>,
    pub(crate) modified_at: Option<Timestamp>,
    pub(crate) trashed_at: Option<Timestamp>,
}

impl Link {
    #[must_use]
    pub fn id(&self) -> &LinkId {
        &self.id
    }

    #[must_use]
    pub fn parent_id(&self) -> Option<&LinkId> {
        self.parent_id.as_ref()
    }

    #[must_use]
    pub fn volume_id(&self) -> Option<&VolumeId> {
        self.volume_id.as_ref()
    }

    #[must_use]
    pub fn kind(&self) -> LinkType {
        self.kind
    }

    #[must_use]
    pub fn state(&self) -> LinkState {
        self.state
    }

    #[must_use]
    pub fn is_folder(&self) -> bool {
        self.kind == LinkType::Folder
    }

    #[must_use]
    pub fn is_file(&self) -> bool {
        self.kind == LinkType::File
    }

    #[must_use]
    pub fn is_album(&self) -> bool {
        self.kind == LinkType::Album
    }

    #[must_use]
    pub fn is_photo(&self) -> bool {
        self.active_revision()
            .and_then(ActiveRevision::photo)
            .is_some()
    }

    #[must_use]
    pub fn is_proton_document(&self) -> bool {
        self.media_type == PROTON_DOCUMENT_MEDIA_TYPE || self.media_type == PROTON_SHEET_MEDIA_TYPE
    }

    #[must_use]
    pub fn encrypted_name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn hash(&self) -> Option<&str> {
        self.hash.as_deref()
    }

    #[must_use]
    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    #[must_use]
    pub fn size(&self) -> i64 {
        self.size
    }

    #[must_use]
    pub fn node_key(&self) -> &str {
        &self.node_key
    }

    #[must_use]
    pub fn node_passphrase(&self) -> &str {
        &self.node_passphrase
    }

    #[must_use]
    pub fn node_passphrase_signature(&self) -> Option<&str> {
        self.node_passphrase_signature.as_deref()
    }

    #[must_use]
    pub fn signature_email(&self) -> Option<&str> {
        signer(self.signature_email.as_deref(), None)
    }

    #[must_use]
    pub fn name_signature_email(&self) -> Option<&str> {
        signer(self.name_signature_email.as_deref(), None).or(self.signature_email())
    }

    #[must_use]
    pub fn extended_attributes(&self) -> Option<&str> {
        self.extended_attributes.as_deref()
    }

    #[must_use]
    pub fn file(&self) -> Option<&FileProperties> {
        self.file.as_ref()
    }

    #[must_use]
    pub fn active_revision(&self) -> Option<&ActiveRevision> {
        self.file.as_ref().and_then(FileProperties::active_revision)
    }

    #[must_use]
    pub fn folder(&self) -> Option<&FolderProperties> {
        self.folder.as_ref()
    }

    #[must_use]
    pub fn album(&self) -> Option<&AlbumProperties> {
        self.album.as_ref()
    }

    #[must_use]
    pub fn photo(&self) -> Option<&PhotoProperties> {
        self.photo.as_ref()
    }

    #[must_use]
    pub fn is_shared(&self) -> bool {
        self.sharing.is_some()
    }

    #[must_use]
    pub fn sharing(&self) -> Option<&SharingDetails> {
        self.sharing.as_ref()
    }

    #[must_use]
    pub fn permissions(&self) -> Option<Role> {
        self.permissions
    }

    #[must_use]
    pub fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }

    #[must_use]
    pub fn created_at(&self) -> Option<Timestamp> {
        self.created_at
    }

    #[must_use]
    pub fn modified_at(&self) -> Option<Timestamp> {
        self.modified_at
    }

    #[must_use]
    pub fn trashed_at(&self) -> Option<Timestamp> {
        self.trashed_at
    }

    pub(crate) fn in_volume(mut self, volume: &VolumeId) -> Link {
        self.volume_id = Some(volume.clone());
        self
    }
}
