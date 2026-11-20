use futures_util::future::try_join_all;
use rikka_proton_api::Outcome;
use serde::Serialize;

use crate::api_errors;
use crate::client::DriveClient;
use crate::crypto;
use crate::error::{Error, Result};
use crate::folder::Folder;
use crate::identifier::{LinkId, VolumeId};
use crate::node::{Node, Parent};
use crate::nodes::{LINKS_PER_REQUEST, LinksRequest};
use crate::share_keys::ShareKeys;

impl DriveClient {
    pub async fn create_folder(
        &self,
        share: &ShareKeys,
        parent: &Folder,
        name: &str,
    ) -> Result<Folder> {
        let signer = crypto::signer(self.keys(), share.address_id())?;
        let email = crypto::email(self.keys(), share.address_id())?;
        let parent_key = parent.keys().key().public_key();

        let new_key = crypto::generate_key(self.pgp(), &parent_key, &signer)?;
        let armored_name = crypto::encrypt_name(self.pgp(), &parent_key, None, &signer, name)?;
        let (_, armored_hash_key) = crypto::generate_hash_key(self.pgp(), &new_key.key)?;

        let request_body = CreateFolderRequest {
            name: &armored_name,
            hash: &parent.hash(name)?,
            parent_link_id: parent.id(),
            node_key: &new_key.armored_key,
            node_passphrase: &new_key.armored_passphrase,
            node_passphrase_signature: &new_key.armored_signature,
            node_hash_key: &armored_hash_key,
            signature_email: &email,
            extended_attributes: None,
        };

        let created: LinkId = self
            .api()
            .post(format!("drive/v2/volumes/{}/folders", share.volume_id()))
            .json(&request_body)
            .send_field_at(&["Folder", "ID"])
            .await
            .map_err(conflict_error)?;

        self.node(share.volume_id(), &created, parent)
            .await?
            .into_folder()
            .ok_or(Error::NotAFolder(created))
    }

    pub async fn rename<'a>(
        &self,
        share: &ShareKeys,
        parent: impl Into<Parent<'a>>,
        node: &Node,
        name: &str,
    ) -> Result<()> {
        let parent = parent.into();
        let signer = crypto::signer(self.keys(), share.address_id())?;
        let email = crypto::email(self.keys(), share.address_id())?;
        let hash = match parent {
            Parent::Share(_) => None,
            Parent::Folder(folder) => Some(folder.hash(name)?),
        };

        let armored_name = crypto::encrypt_name(
            self.pgp(),
            &parent.key().public_key(),
            Some(node.keys().name_session_key()),
            &signer,
            name,
        )?;

        let request_body = RenameRequest {
            name: &armored_name,
            hash: hash.as_deref(),
            original_hash: node.link().hash(),
            name_signature_email: &email,
        };

        self.api()
            .put(format!(
                "drive/v2/volumes/{}/links/{}/rename",
                share.volume_id(),
                node.id()
            ))
            .json(&request_body)
            .send_unit()
            .await
            .map_err(conflict_error)?;

        Ok(())
    }

    pub async fn move_node(&self, share: &ShareKeys, node: &Node, parent: &Folder) -> Result<()> {
        let signer = crypto::signer(self.keys(), share.address_id())?;
        let email = crypto::email(self.keys(), share.address_id())?;
        let parent_key = parent.keys().key().public_key();
        let original_hash = node
            .link()
            .hash()
            .ok_or_else(|| Error::Incomplete("original link hash"))?;

        let armored_name = crypto::encrypt_name(
            self.pgp(),
            &parent_key,
            Some(node.keys().name_session_key()),
            &signer,
            node.name(),
        )?;
        let (armored_passphrase, armored_signature) = crypto::encrypt_passphrase(
            self.pgp(),
            &parent_key,
            node.keys().passphrase_session_key(),
            &signer,
            node.keys().passphrase(),
        )?;
        let unsigned = node.signatures().passphrase().is_missing();
        let content_hash = match node.as_file() {
            Some(file) if file.photo().is_some() => file
                .sha1()
                .map(|sha1| parent.hash(&sha1.to_ascii_lowercase()))
                .transpose()?,
            _ => None,
        };

        let request_body = MoveRequest {
            name: &armored_name,
            hash: &parent.hash(node.name())?,
            original_hash,
            parent_link_id: parent.id(),
            node_passphrase: &armored_passphrase,
            node_passphrase_signature: unsigned.then_some(armored_signature.as_str()),
            name_signature_email: &email,
            signature_email: unsigned.then_some(email.as_str()),
            content_hash: content_hash.as_deref(),
        };

        self.api()
            .put(format!(
                "drive/v2/volumes/{}/links/{}/move",
                share.volume_id(),
                node.id()
            ))
            .json(&request_body)
            .send_unit()
            .await
            .map_err(conflict_error)?;

        Ok(())
    }

    pub async fn trash(&self, volume: &VolumeId, links: &[LinkId]) -> Result<Vec<Outcome<LinkId>>> {
        self.send_link_batches(
            format!("drive/v2/volumes/{volume}/trash_multiple"),
            links,
            HttpMethod::Post,
        )
        .await
    }

    pub async fn restore(
        &self,
        volume: &VolumeId,
        links: &[LinkId],
    ) -> Result<Vec<Outcome<LinkId>>> {
        self.send_link_batches(
            format!("drive/v2/volumes/{volume}/trash/restore_multiple"),
            links,
            HttpMethod::Put,
        )
        .await
    }

    pub async fn delete(
        &self,
        volume: &VolumeId,
        links: &[LinkId],
    ) -> Result<Vec<Outcome<LinkId>>> {
        self.send_link_batches(
            format!("drive/v2/volumes/{volume}/trash/delete_multiple"),
            links,
            HttpMethod::Post,
        )
        .await
    }

    pub async fn delete_drafts(
        &self,
        volume: &VolumeId,
        links: &[LinkId],
    ) -> Result<Vec<Outcome<LinkId>>> {
        self.send_link_batches(
            format!("drive/v2/volumes/{volume}/delete_multiple"),
            links,
            HttpMethod::Post,
        )
        .await
    }

    async fn send_link_batches(
        &self,
        path: String,
        links: &[LinkId],
        verb: HttpMethod,
    ) -> Result<Vec<Outcome<LinkId>>> {
        let requests = links.chunks(LINKS_PER_REQUEST).map(|batch| {
            let request = match verb {
                HttpMethod::Post => self.api().post(&path),
                HttpMethod::Put => self.api().put(&path),
            };
            request
                .json(&LinksRequest { link_ids: batch })
                .send_field_or_default::<Vec<Outcome<LinkId>>>("Responses")
        });
        let response = try_join_all(requests).await?;

        Ok(response.into_iter().flatten().collect())
    }
}

pub(crate) fn conflict_error(error: rikka_proton_api::Error) -> Error {
    match api_errors::conflict(&error) {
        Some(conflict) => Error::Conflict {
            link: conflict.link,
            draft: conflict.draft_revision,
        },
        None => error.into(),
    }
}

#[derive(Debug, Clone, Copy)]
enum HttpMethod {
    Post,
    Put,
}

#[derive(Debug, Serialize)]
struct CreateFolderRequest<'a> {
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
    #[serde(rename = "NodeHashKey")]
    node_hash_key: &'a str,
    #[serde(rename = "SignatureEmail")]
    signature_email: &'a str,
    #[serde(rename = "XAttr")]
    extended_attributes: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct RenameRequest<'a> {
    #[serde(rename = "Name")]
    name: &'a str,
    #[serde(rename = "Hash")]
    hash: Option<&'a str>,
    #[serde(rename = "OriginalHash")]
    original_hash: Option<&'a str>,
    #[serde(rename = "NameSignatureEmail")]
    name_signature_email: &'a str,
}

#[derive(Debug, Serialize)]
struct MoveRequest<'a> {
    #[serde(rename = "Name")]
    name: &'a str,
    #[serde(rename = "Hash")]
    hash: &'a str,
    #[serde(rename = "OriginalHash")]
    original_hash: &'a str,
    #[serde(rename = "ParentLinkID")]
    parent_link_id: &'a LinkId,
    #[serde(rename = "NodePassphrase")]
    node_passphrase: &'a str,
    #[serde(rename = "NodePassphraseSignature")]
    node_passphrase_signature: Option<&'a str>,
    #[serde(rename = "NameSignatureEmail")]
    name_signature_email: &'a str,
    #[serde(rename = "SignatureEmail")]
    signature_email: Option<&'a str>,
    #[serde(rename = "ContentHash")]
    content_hash: Option<&'a str>,
}
