use rikka_proton_pgp::{Pgp, PrivateKey};
use serde::Serialize;

use crate::crypto;
use crate::error::{Error, Result};
use crate::file::File;
use crate::folder::Folder;
use crate::identifier::{LinkId, VolumeId};

#[derive(Debug, Clone)]
pub(super) struct Sealed {
    pub(super) link_id: LinkId,
    pub(super) hash: String,
    pub(super) original_hash: Option<String>,
    pub(super) name: String,
    pub(super) node_passphrase: String,
    pub(super) node_passphrase_signature: Option<String>,
    pub(super) content_hash: String,
}

#[derive(Debug, Clone)]
pub(super) struct Payload {
    pub(super) volume: VolumeId,
    pub(super) main: Sealed,
    pub(super) related: Vec<Sealed>,
}

impl Payload {
    pub(super) fn size(&self) -> usize {
        1 + self.related.len()
    }

    pub(super) fn all(&self) -> impl Iterator<Item = &Sealed> {
        std::iter::once(&self.main).chain(&self.related)
    }
}

pub(super) struct Sealing<'a> {
    pub(super) pgp: Pgp,
    pub(super) target: &'a Folder,
    pub(super) signer: &'a PrivateKey,
}

impl Sealing<'_> {
    pub(super) fn sealed(&self, photo: &File) -> Result<Sealed> {
        let target_key = self.target.keys().key().public_key();
        let sha1 = photo.sha1().ok_or(Error::Incomplete("photo SHA-1"))?;
        let name = crypto::encrypt_name(
            &self.pgp,
            &target_key,
            Some(photo.keys().name_session_key()),
            self.signer,
            photo.name(),
        )?;
        let (node_passphrase, signature) = crypto::encrypt_passphrase(
            &self.pgp,
            &target_key,
            photo.keys().passphrase_session_key(),
            self.signer,
            photo.keys().passphrase(),
        )?;
        let unsigned = photo.signatures().passphrase().is_missing();

        Ok(Sealed {
            link_id: photo.id().clone(),
            hash: self.target.hash(photo.name())?,
            original_hash: photo.link().hash().map(str::to_owned),
            name,
            node_passphrase,
            node_passphrase_signature: unsigned.then_some(signature),
            content_hash: self.target.hash(&sha1.to_ascii_lowercase())?,
        })
    }
}

#[derive(Debug, Serialize)]
pub(super) struct AlbumEntry<'a> {
    #[serde(rename = "LinkID")]
    link_id: &'a LinkId,
    #[serde(rename = "Hash")]
    hash: &'a str,
    #[serde(rename = "Name")]
    name: &'a str,
    #[serde(rename = "NameSignatureEmail")]
    name_signature_email: &'a str,
    #[serde(rename = "NodePassphrase")]
    node_passphrase: &'a str,
    #[serde(rename = "ContentHash")]
    content_hash: &'a str,
    #[serde(rename = "NodePassphraseSignature")]
    node_passphrase_signature: Option<&'a str>,
    #[serde(rename = "SignatureEmail")]
    signature_email: Option<&'a str>,
}

impl<'a> AlbumEntry<'a> {
    pub(super) fn new(sealed: &'a Sealed, email: &'a str) -> AlbumEntry<'a> {
        AlbumEntry {
            link_id: &sealed.link_id,
            hash: &sealed.hash,
            name: &sealed.name,
            name_signature_email: email,
            node_passphrase: &sealed.node_passphrase,
            content_hash: &sealed.content_hash,
            node_passphrase_signature: sealed.node_passphrase_signature.as_deref(),
            signature_email: sealed.node_passphrase_signature.as_ref().map(|_| email),
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct TransferEntry<'a> {
    #[serde(rename = "LinkID")]
    link_id: &'a LinkId,
    #[serde(rename = "Hash")]
    hash: &'a str,
    #[serde(rename = "OriginalHash")]
    original_hash: Option<&'a str>,
    #[serde(rename = "Name")]
    name: &'a str,
    #[serde(rename = "NodePassphrase")]
    node_passphrase: &'a str,
    #[serde(rename = "ContentHash")]
    content_hash: &'a str,
    #[serde(rename = "NodePassphraseSignature")]
    node_passphrase_signature: Option<&'a str>,
}

impl<'a> From<&'a Sealed> for TransferEntry<'a> {
    fn from(sealed: &'a Sealed) -> TransferEntry<'a> {
        TransferEntry {
            link_id: &sealed.link_id,
            hash: &sealed.hash,
            original_hash: sealed.original_hash.as_deref(),
            name: &sealed.name,
            node_passphrase: &sealed.node_passphrase,
            content_hash: &sealed.content_hash,
            node_passphrase_signature: sealed.node_passphrase_signature.as_deref(),
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct RelatedCopy<'a> {
    #[serde(rename = "LinkID")]
    link_id: &'a LinkId,
    #[serde(rename = "Hash")]
    hash: &'a str,
    #[serde(rename = "Name")]
    name: &'a str,
    #[serde(rename = "NodePassphrase")]
    node_passphrase: &'a str,
    #[serde(rename = "ContentHash")]
    content_hash: &'a str,
}

impl<'a> From<&'a Sealed> for RelatedCopy<'a> {
    fn from(sealed: &'a Sealed) -> RelatedCopy<'a> {
        RelatedCopy {
            link_id: &sealed.link_id,
            hash: &sealed.hash,
            name: &sealed.name,
            node_passphrase: &sealed.node_passphrase,
            content_hash: &sealed.content_hash,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct CopyRequest<'a> {
    #[serde(rename = "TargetVolumeID")]
    pub(super) target_volume_id: &'a VolumeId,
    #[serde(rename = "TargetParentLinkID")]
    pub(super) target_parent_link_id: &'a LinkId,
    #[serde(rename = "Hash")]
    pub(super) hash: &'a str,
    #[serde(rename = "Name")]
    pub(super) name: &'a str,
    #[serde(rename = "NameSignatureEmail")]
    pub(super) name_signature_email: &'a str,
    #[serde(rename = "NodePassphrase")]
    pub(super) node_passphrase: &'a str,
    #[serde(rename = "NodePassphraseSignature")]
    pub(super) node_passphrase_signature: Option<&'a str>,
    #[serde(rename = "SignatureEmail")]
    pub(super) signature_email: Option<&'a str>,
    #[serde(rename = "Photos")]
    pub(super) photos: CopiedPhotos<'a>,
}

#[derive(Debug, Serialize)]
pub(super) struct CopiedPhotos<'a> {
    #[serde(rename = "ContentHash")]
    pub(super) content_hash: &'a str,
    #[serde(rename = "RelatedPhotos")]
    pub(super) related: Vec<RelatedCopy<'a>>,
}

impl<'a> CopyRequest<'a> {
    pub(super) fn new(
        payload: &'a Payload,
        target_volume: &'a VolumeId,
        target: &'a LinkId,
        email: &'a str,
    ) -> CopyRequest<'a> {
        let main = &payload.main;
        CopyRequest {
            target_volume_id: target_volume,
            target_parent_link_id: target,
            hash: &main.hash,
            name: &main.name,
            name_signature_email: email,
            node_passphrase: &main.node_passphrase,
            node_passphrase_signature: main.node_passphrase_signature.as_deref(),
            signature_email: main.node_passphrase_signature.as_ref().map(|_| email),
            photos: CopiedPhotos {
                content_hash: &main.content_hash,
                related: payload.related.iter().map(RelatedCopy::from).collect(),
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct FavoriteData<'a> {
    #[serde(flatten)]
    main: AlbumEntryFields<'a>,
    #[serde(rename = "RelatedPhotos")]
    related: Vec<AlbumEntry<'a>>,
}

#[derive(Debug, Serialize)]
struct AlbumEntryFields<'a> {
    #[serde(rename = "Hash")]
    hash: &'a str,
    #[serde(rename = "Name")]
    name: &'a str,
    #[serde(rename = "NameSignatureEmail")]
    name_signature_email: &'a str,
    #[serde(rename = "NodePassphrase")]
    node_passphrase: &'a str,
    #[serde(rename = "ContentHash")]
    content_hash: &'a str,
    #[serde(rename = "NodePassphraseSignature")]
    node_passphrase_signature: Option<&'a str>,
    #[serde(rename = "SignatureEmail")]
    signature_email: Option<&'a str>,
}

impl<'a> FavoriteData<'a> {
    pub(super) fn new(payload: &'a Payload, email: &'a str) -> FavoriteData<'a> {
        let main = &payload.main;
        FavoriteData {
            main: AlbumEntryFields {
                hash: &main.hash,
                name: &main.name,
                name_signature_email: email,
                node_passphrase: &main.node_passphrase,
                content_hash: &main.content_hash,
                node_passphrase_signature: main.node_passphrase_signature.as_deref(),
                signature_email: main.node_passphrase_signature.as_ref().map(|_| email),
            },
            related: payload
                .related
                .iter()
                .map(|sealed| AlbumEntry::new(sealed, email))
                .collect(),
        }
    }
}
