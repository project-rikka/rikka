use jiff::Timestamp;
use rikka_proton_account::AddressId;
use serde::{Deserialize, Serialize};

use crate::identifier::{LinkId, RevisionId, VolumeId};
use crate::photo_properties::PhotoTag;
use crate::thumbnail::ThumbnailType;

#[derive(Debug, Serialize)]
pub(super) struct CreateFileRequest<'a> {
    #[serde(rename = "Name")]
    pub(super) name: &'a str,
    #[serde(rename = "Hash")]
    pub(super) hash: &'a str,
    #[serde(rename = "MIMEType")]
    pub(super) media_type: &'a str,
    #[serde(rename = "ParentLinkID")]
    pub(super) parent_link_id: &'a LinkId,
    #[serde(rename = "NodeKey")]
    pub(super) node_key: &'a str,
    #[serde(rename = "NodePassphrase")]
    pub(super) node_passphrase: &'a str,
    #[serde(rename = "NodePassphraseSignature")]
    pub(super) node_passphrase_signature: &'a str,
    #[serde(rename = "ContentKeyPacket")]
    pub(super) content_key_packet: &'a str,
    #[serde(rename = "ContentKeyPacketSignature")]
    pub(super) content_key_packet_signature: &'a str,
    #[serde(rename = "SignatureAddress")]
    pub(super) signature_address: &'a str,
    #[serde(rename = "ClientUID")]
    pub(super) client_uid: Option<&'a str>,
}

#[derive(Debug, Serialize)]
pub(super) struct CreateRevisionRequest<'a> {
    #[serde(rename = "CurrentRevisionID")]
    pub(super) current_revision_id: Option<&'a RevisionId>,
    #[serde(rename = "ClientUID")]
    pub(super) client_uid: Option<&'a str>,
}

#[derive(Debug, Serialize)]
pub(super) struct BlocksRequest<'a> {
    #[serde(rename = "AddressID")]
    pub(super) address_id: &'a AddressId,
    #[serde(rename = "VolumeID")]
    pub(super) volume_id: &'a VolumeId,
    #[serde(rename = "LinkID")]
    pub(super) link_id: &'a LinkId,
    #[serde(rename = "RevisionID")]
    pub(super) revision_id: &'a RevisionId,
    #[serde(rename = "BlockList")]
    pub(super) block_list: Vec<BlockRequest<'a>>,
    #[serde(rename = "ThumbnailList")]
    pub(super) thumbnail_list: Vec<ThumbnailRequest>,
}

#[derive(Debug, Serialize)]
pub(super) struct ThumbnailRequest {
    #[serde(rename = "Type")]
    pub(super) kind: i32,
    #[serde(rename = "Size")]
    pub(super) size: u64,
    #[serde(rename = "Hash")]
    pub(super) hash: String,
}

#[derive(Debug, Serialize)]
pub(super) struct BlockRequest<'a> {
    #[serde(rename = "Index")]
    pub(super) index: u32,
    #[serde(rename = "Size")]
    pub(super) size: u64,
    #[serde(rename = "Hash")]
    pub(super) hash: String,
    #[serde(rename = "EncSignature")]
    pub(super) encrypted_signature: &'a str,
    #[serde(rename = "Verifier")]
    pub(super) verifier: VerifierToken,
}

#[derive(Debug, Serialize)]
pub(super) struct VerifierToken {
    #[serde(rename = "Token")]
    pub(super) token: String,
}

#[derive(Debug, Serialize)]
pub(super) struct CommitRequest<'a> {
    #[serde(rename = "ManifestSignature")]
    pub(super) manifest_signature: &'a str,
    #[serde(rename = "SignatureAddress")]
    pub(super) signature_address: &'a str,
    #[serde(rename = "XAttr")]
    pub(super) extended_attributes: &'a str,
    #[serde(rename = "ChecksumVerified")]
    pub(super) checksum_verified: bool,
    #[serde(rename = "Photo")]
    pub(super) photo: Option<PhotoCommit<'a>>,
}

#[derive(Debug, Serialize)]
pub(super) struct PhotoCommit<'a> {
    #[serde(rename = "CaptureTime", with = "rikka_proton_api::unix_seconds")]
    pub(super) capture_time: Timestamp,
    #[serde(rename = "ContentHash")]
    pub(super) content_hash: String,
    #[serde(rename = "MainPhotoLinkID")]
    pub(super) main_photo_link_id: Option<&'a LinkId>,
    #[serde(rename = "Exif")]
    pub(super) exif: Option<&'a str>,
    #[serde(rename = "Tags")]
    pub(super) tags: &'a [PhotoTag],
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateFileResponse {
    #[serde(rename = "File")]
    pub(super) file: CreatedFile,
}

#[derive(Debug, Deserialize)]
pub(super) struct CreatedFile {
    #[serde(rename = "ID")]
    pub(super) id: LinkId,
    #[serde(rename = "RevisionID")]
    pub(super) revision_id: RevisionId,
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateRevisionResponse {
    #[serde(rename = "Revision")]
    pub(super) revision: CreatedRevision,
}

#[derive(Debug, Deserialize)]
pub(super) struct CreatedRevision {
    #[serde(rename = "ID")]
    pub(super) id: RevisionId,
}

#[derive(Debug, Deserialize)]
pub(super) struct VerificationResponse {
    #[serde(rename = "VerificationCode")]
    pub(super) code: String,
    #[serde(rename = "ContentKeyPacket")]
    pub(super) content_key_packet: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct BlocksResponse {
    #[serde(rename = "UploadLinks", default)]
    pub(super) upload_links: Vec<UploadLink>,
    #[serde(rename = "ThumbnailLinks", default)]
    pub(super) thumbnail_links: Vec<UploadLink>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UploadLink {
    #[serde(rename = "Index", default)]
    pub(super) index: Option<u32>,
    #[serde(rename = "ThumbnailType", default)]
    pub(super) thumbnail_type: Option<ThumbnailType>,
    #[serde(rename = "Token", default)]
    pub(super) token: String,
    #[serde(rename = "BareURL", default)]
    pub(super) bare_url: String,
    #[serde(rename = "URL", default)]
    pub(super) url: String,
}

impl UploadLink {
    pub(super) fn address(&self) -> &str {
        match self.bare_url.is_empty() {
            true => &self.url,
            false => &self.bare_url,
        }
    }
}
