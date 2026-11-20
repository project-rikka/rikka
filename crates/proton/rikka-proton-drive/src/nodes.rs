use futures_util::future::try_join_all;
use futures_util::{Stream, TryStreamExt as _};
use rikka_proton_api::{offload, parallel_map_each};
use rikka_proton_cache::Wired;
use rikka_proton_pgp::{PrivateKey, PublicKey, SignatureStatus};
use serde::{Deserialize, Serialize};

use crate::anchor::{ANCHOR, Anchor, anchored};
use crate::client::DriveClient;
use crate::crypto;
use crate::error::{Error, Result};
use crate::file::File;
use crate::folder::Folder;
use crate::identifier::{LinkId, VolumeId};
use crate::link::{FolderProperties, Link, LinkType};
use crate::node::{Node, NodeParts, Parent};
use crate::node_keys::NodeKeys;
use crate::photo_properties::AlbumProperties;
use crate::share_keys::ShareKeys;
use crate::signatures::Signatures;

pub(crate) const LINKS_PER_REQUEST: usize = 100;

impl DriveClient {
    pub async fn link(&self, volume: &VolumeId, link: &LinkId) -> Result<Link> {
        self.links(volume, std::slice::from_ref(link))
            .await?
            .into_iter()
            .find(|child| child.id() == link)
            .ok_or_else(|| Error::LinkNotFound(link.clone()))
    }

    pub async fn links(&self, volume: &VolumeId, links: &[LinkId]) -> Result<Vec<Link>> {
        self.fetch_links(format!("drive/v2/volumes/{volume}/links"), volume, links)
            .await
    }

    pub async fn photo_links(&self, volume: &VolumeId, links: &[LinkId]) -> Result<Vec<Link>> {
        self.fetch_links(
            format!("drive/photos/volumes/{volume}/links"),
            volume,
            links,
        )
        .await
    }

    pub async fn child_ids(&self, volume: &VolumeId, folder: &LinkId) -> Result<Vec<LinkId>> {
        let collected: Vec<LinkId> = self
            .child_id_pages(volume, folder, None)
            .try_concat()
            .await?;
        self.cache_children(folder, &collected);
        Ok(collected)
    }

    pub(crate) fn child_id_pages(
        &self,
        volume: &VolumeId,
        folder: &LinkId,
        after: Option<LinkId>,
    ) -> impl Stream<Item = Result<Vec<LinkId>>> + Send + 'static {
        let paging = match after {
            Some(anchor) => anchored().starting_from(anchor),
            None => anchored(),
        };
        self.api()
            .get(children_path(volume, folder))
            .pages(paging)
            .stream(|page: ChildPage| page.anchor.page(page.ids))
            .map_err(Error::from)
    }

    pub async fn root(&self, share: &ShareKeys) -> Result<Folder> {
        let link = self.link(share.volume_id(), share.root_id()).await?;
        let id = link.id().clone();

        self.decrypt_link_offloaded(link, share.key().clone())
            .await?
            .into_folder()
            .ok_or(Error::NotAFolder(id))
    }

    pub async fn node<'a>(
        &self,
        volume: &VolumeId,
        link: &LinkId,
        parent: impl Into<Parent<'a>>,
    ) -> Result<Node> {
        let parent_key = parent.into().key().clone();
        let link = self.link(volume, link).await?;

        self.decrypt_link_offloaded(link, parent_key).await
    }

    pub async fn folder<'a>(
        &self,
        volume: &VolumeId,
        link: &LinkId,
        parent: impl Into<Parent<'a>>,
    ) -> Result<Folder> {
        self.node(volume, link, parent)
            .await?
            .into_folder()
            .ok_or_else(|| Error::NotAFolder(link.clone()))
    }

    pub async fn file<'a>(
        &self,
        volume: &VolumeId,
        link: &LinkId,
        parent: impl Into<Parent<'a>>,
    ) -> Result<File> {
        self.node(volume, link, parent)
            .await?
            .into_file()
            .ok_or_else(|| Error::NotAFile(link.clone()))
    }

    pub async fn children(&self, volume: &VolumeId, folder: &Folder) -> Result<Vec<Node>> {
        self.listing(volume, folder).all().await
    }

    pub(crate) async fn child_page(
        &self,
        volume: &VolumeId,
        folder: &LinkId,
        anchor: Option<&LinkId>,
    ) -> Result<ChildPage> {
        let mut request = self.api().get(children_path(volume, folder));
        if let Some(anchor) = anchor {
            request = request.query(&[(ANCHOR, anchor.as_str())]);
        }

        Ok(request.send().await?)
    }

    async fn fetch_links(
        &self,
        path: String,
        volume: &VolumeId,
        links: &[LinkId],
    ) -> Result<Vec<Link>> {
        let request = links.chunks(LINKS_PER_REQUEST).map(|batch| {
            self.api()
                .post(&path)
                .json(&LinksRequest { link_ids: batch })
                .send_field_or_default::<Vec<Wired<Link>>>("Links")
        });
        let response = try_join_all(request).await?;
        let wired: Vec<Wired<Link>> = response.into_iter().flatten().collect();
        self.cache_links(volume, &wired);

        Ok(wired
            .into_iter()
            .map(|link| link.into_value().in_volume(volume))
            .collect())
    }

    pub async fn decrypt_links<'a>(
        &self,
        links: Vec<Link>,
        parent: impl Into<Parent<'a>>,
    ) -> Result<Vec<Node>> {
        let parent_key = parent.into().key().clone();

        self.decrypt_links_with_key(links, parent_key).await
    }

    pub fn decrypt_link<'a>(&self, link: Link, parent: impl Into<Parent<'a>>) -> Result<Node> {
        self.decrypt_link_with_key(link, parent.into().key())
    }

    pub(crate) async fn decrypt_links_with_key(
        &self,
        links: Vec<Link>,
        parent_key: PrivateKey,
    ) -> Result<Vec<Node>> {
        let client = self.clone();

        parallel_map_each(links, move |link| {
            client.decrypt_link_with_key(link, &parent_key)
        })
        .await
        .into_iter()
        .collect()
    }

    pub(crate) async fn decrypt_link_offloaded(
        &self,
        link: Link,
        parent_key: PrivateKey,
    ) -> Result<Node> {
        let client = self.clone();

        offload(move || client.decrypt_link_with_key(link, &parent_key)).await
    }

    pub(crate) fn decrypt_link_with_key(
        &self,
        link: Link,
        parent_key: &PrivateKey,
    ) -> Result<Node> {
        let pgp = self.pgp();

        let key_verification =
            self.verification_keys_or(link.signature_email(), parent_key.public_key());
        let name_verification = match link.name_signature_email() == link.signature_email() {
            true => key_verification.clone(),
            false => {
                self.verification_keys_or(link.name_signature_email(), parent_key.public_key())
            }
        };

        let node_key_material = crypto::decrypt_key(
            pgp,
            link.node_key(),
            link.node_passphrase(),
            link.node_passphrase_signature(),
            std::slice::from_ref(parent_key),
            &key_verification,
        )?;
        let passphrase = Error::check_signature("node passphrase", node_key_material.signature)?;

        let decrypted_name =
            crypto::decrypt_name(pgp, parent_key, &name_verification, link.encrypted_name())?;
        let name_signature = Error::check_signature("node name", decrypted_name.signature)?;

        let node_key = node_key_material.key;
        let mut keys = NodeKeys::new(
            node_key.clone(),
            node_key_material.passphrase,
            node_key_material.session_key,
            decrypted_name.session_key,
        );

        let mut content = SignatureStatus::Missing;
        match link.kind() {
            LinkType::Folder | LinkType::Album => {
                let armored = match link.kind() {
                    LinkType::Album => link.album().and_then(AlbumProperties::node_hash_key),
                    _ => link.folder().and_then(FolderProperties::node_hash_key),
                };
                if let Some(armored) = armored {
                    let (hash_key, signature) =
                        crypto::decrypt_hash_key(pgp, &node_key, &key_verification, armored)?;
                    content = Error::check_signature("node hash key", signature)?;
                    keys = keys.with_hash_key(hash_key);
                }
            }
            LinkType::File => {
                let file = link.file().ok_or(Error::Incomplete("file properties"))?;
                let packet = file
                    .content_key_packet()
                    .ok_or(Error::Incomplete("content key packet"))?;
                let (content_key, signature) = crypto::decrypt_content_key(
                    pgp,
                    &node_key,
                    &key_verification,
                    packet,
                    file.content_key_packet_signature(),
                )?;
                content = Error::check_signature("content key packet", signature)?;
                keys = keys.with_content_key(content_key);
            }
            LinkType::Other(_) => {
                return Err(Error::UnknownNode(link.id().clone()));
            }
        }

        let mut attributes = None;
        let mut attributes_signature = SignatureStatus::Missing;
        if let Some(armored) = link.extended_attributes() {
            let (decrypted, signature) =
                crypto::decrypt_attributes(pgp, &node_key, &key_verification, armored)?;
            attributes_signature = Error::check_signature("extended attributes", signature)?;
            attributes = Some(decrypted);
        }

        let parts = NodeParts {
            signatures: Signatures::new(passphrase, name_signature, content, attributes_signature),
            link,
            name: decrypted_name.name,
            keys,
            attributes,
        };

        Ok(match parts.link.kind() {
            LinkType::File => Node::File(File::new(parts)),
            _ => Node::Folder(Folder::new(parts)),
        })
    }

    pub(crate) fn verification_keys_or(
        &self,
        email: Option<&str>,
        fallback: PublicKey,
    ) -> Vec<PublicKey> {
        match crypto::verification_keys(self.keys(), email) {
            keys if keys.is_empty() => vec![fallback],
            keys => keys,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChildPage {
    #[serde(rename = "LinkIDs", default)]
    pub(crate) ids: Vec<LinkId>,
    #[serde(flatten)]
    anchor: Anchor<LinkId>,
}

impl ChildPage {
    pub(crate) fn next(&self) -> Option<&LinkId> {
        self.anchor.next()
    }
}

fn children_path(volume: &VolumeId, folder: &LinkId) -> String {
    format!("drive/v2/volumes/{volume}/folders/{folder}/children")
}

#[derive(Debug, Serialize)]
pub(crate) struct LinksRequest<'a> {
    #[serde(rename = "LinkIDs")]
    pub(crate) link_ids: &'a [LinkId],
}
