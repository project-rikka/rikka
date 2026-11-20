use rikka_proton_api::{percent, wire_enum};
use serde::Serialize;

use crate::client::DriveClient;
use crate::crypto;
use crate::error::{Error, Result};
use crate::file::File;
use crate::folder::Folder;
use crate::identifier::{LinkId, VolumeId};
use crate::link::{PROTON_DOCUMENT_MEDIA_TYPE, PROTON_SHEET_MEDIA_TYPE};
use crate::share_keys::ShareKeys;

const DOCS_ORIGIN: &str = "https://docs.proton.me";

wire_enum! {
    pub enum DocumentType: default Document {
        Document = 1,
        Spreadsheet = 2,
    }
}

impl DocumentType {
    #[must_use]
    pub fn from_file(file: &File) -> Option<DocumentType> {
        match file.media_type() {
            PROTON_DOCUMENT_MEDIA_TYPE => Some(DocumentType::Document),
            PROTON_SHEET_MEDIA_TYPE => Some(DocumentType::Spreadsheet),
            _ => None,
        }
    }

    fn path(self) -> &'static str {
        match self {
            DocumentType::Spreadsheet => "sheet",
            DocumentType::Document | DocumentType::Other(_) => "doc",
        }
    }
}

impl DriveClient {
    pub async fn create_document(
        &self,
        share: &ShareKeys,
        parent: &Folder,
        name: &str,
        kind: DocumentType,
    ) -> Result<File> {
        let pgp = self.pgp();
        let signer = crypto::signer(self.keys(), share.address_id())?;
        let email = crypto::email(self.keys(), share.address_id())?;
        let parent_key = parent.keys().key().public_key();

        let new_key = crypto::generate_key(pgp, &parent_key, &signer)?;
        let armored_name = crypto::encrypt_name(pgp, &parent_key, None, &signer, name)?;
        let (_, packet, packet_signature) = crypto::generate_content_key(pgp, &new_key.key)?;
        let manifest_signature = crypto::sign_manifest(pgp, &signer, &[])?;

        let request = CreateDocumentRequest {
            content_key_packet: &packet,
            content_key_packet_signature: &packet_signature,
            manifest_signature: &manifest_signature,
            kind,
            name: &armored_name,
            hash: &parent.hash(name)?,
            parent_link_id: parent.id(),
            node_key: &new_key.armored_key,
            node_passphrase: &new_key.armored_passphrase,
            node_passphrase_signature: &new_key.armored_signature,
            signature_address: &email,
        };
        let created: LinkId = self
            .api()
            .post(format!("drive/v2/volumes/{}/documents", share.volume_id()))
            .json(&request)
            .send_field_at(&["Document", "LinkID"])
            .await?;

        self.node(share.volume_id(), &created, parent)
            .await?
            .into_file()
            .ok_or(Error::NotAFile(created))
    }
}

impl File {
    pub fn document_url(&self, volume: &VolumeId) -> Result<String> {
        let kind =
            DocumentType::from_file(self).ok_or_else(|| Error::NotADocument(self.id().clone()))?;

        Ok(format!(
            "{DOCS_ORIGIN}/{}?mode=open&volumeId={}&linkId={}",
            kind.path(),
            percent::encode(volume.as_str()),
            percent::encode(self.id().as_str()),
        ))
    }
}

#[derive(Debug, Serialize)]
struct CreateDocumentRequest<'a> {
    #[serde(rename = "ContentKeyPacket")]
    content_key_packet: &'a str,
    #[serde(rename = "ContentKeyPacketSignature")]
    content_key_packet_signature: &'a str,
    #[serde(rename = "ManifestSignature")]
    manifest_signature: &'a str,
    #[serde(rename = "DocumentType")]
    kind: DocumentType,
    #[serde(rename = "Name")]
    name: &'a str,
    #[serde(rename = "Hash")]
    hash: &'a str,
    #[serde(rename = "ParentLinkID")]
    parent_link_id: &'a LinkId,
    #[serde(rename = "NodeKey")]
    node_key: &'a str,
    #[serde(rename = "NodePassphrase")]
    node_passphrase: &'a str,
    #[serde(rename = "NodePassphraseSignature")]
    node_passphrase_signature: &'a str,
    #[serde(rename = "SignatureAddress")]
    signature_address: &'a str,
}
