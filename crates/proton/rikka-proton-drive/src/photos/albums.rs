use jiff::Timestamp;
use rikka_proton_api::Outcome;
use serde::{Deserialize, Serialize};

use crate::anchor::{Anchor, anchored};
use crate::api_errors;
use crate::client::DriveClient;
use crate::crypto;
use crate::error::{Error, Result};
use crate::folder::Folder;
use crate::identifier::{LinkId, ShareId, VolumeId};
use crate::photo_properties::PhotoTag;
use crate::photos::library::PhotoLibrary;
use crate::photos::timeline::RelatedPhoto;

const REMOVALS_PER_REQUEST: usize = 10;

#[derive(Debug, Clone, Deserialize)]
pub struct AlbumSummary {
    #[serde(rename = "LinkID")]
    link_id: LinkId,
    #[serde(rename = "VolumeID")]
    volume_id: VolumeId,
    #[serde(rename = "ShareID", default)]
    share_id: Option<ShareId>,
    #[serde(rename = "CoverLinkID", default)]
    cover_id: Option<LinkId>,
    #[serde(rename = "PhotoCount", default)]
    photo_count: u64,
    #[serde(rename = "Locked", default)]
    locked: bool,
    #[serde(
        rename = "LastActivityTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    last_activity_at: Option<Timestamp>,
}

impl AlbumSummary {
    #[must_use]
    pub fn link_id(&self) -> &LinkId {
        &self.link_id
    }

    #[must_use]
    pub fn volume_id(&self) -> &VolumeId {
        &self.volume_id
    }

    #[must_use]
    pub fn share_id(&self) -> Option<&ShareId> {
        self.share_id.as_ref()
    }

    #[must_use]
    pub fn cover_id(&self) -> Option<&LinkId> {
        self.cover_id.as_ref()
    }

    #[must_use]
    pub fn photo_count(&self) -> u64 {
        self.photo_count
    }

    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    #[must_use]
    pub fn last_activity_at(&self) -> Option<Timestamp> {
        self.last_activity_at
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlbumPhoto {
    #[serde(rename = "LinkID")]
    link_id: LinkId,
    #[serde(
        rename = "CaptureTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    captured_at: Option<Timestamp>,
    #[serde(
        rename = "AddedTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    added_at: Option<Timestamp>,
    #[serde(rename = "Hash", default)]
    hash: Option<String>,
    #[serde(rename = "ContentHash", default)]
    content_hash: Option<String>,
    #[serde(rename = "IsChildOfAlbum", default)]
    child_of_album: bool,
    #[serde(rename = "Tags", default)]
    tags: Vec<PhotoTag>,
    #[serde(rename = "RelatedPhotos", default)]
    related: Vec<RelatedPhoto>,
}

impl AlbumPhoto {
    #[must_use]
    pub fn link_id(&self) -> &LinkId {
        &self.link_id
    }

    #[must_use]
    pub fn captured_at(&self) -> Option<Timestamp> {
        self.captured_at
    }

    #[must_use]
    pub fn added_at(&self) -> Option<Timestamp> {
        self.added_at
    }

    #[must_use]
    pub fn hash(&self) -> Option<&str> {
        self.hash.as_deref()
    }

    #[must_use]
    pub fn content_hash(&self) -> Option<&str> {
        self.content_hash.as_deref()
    }

    #[must_use]
    pub fn is_child_of_album(&self) -> bool {
        self.child_of_album
    }

    #[must_use]
    pub fn tags(&self) -> &[PhotoTag] {
        &self.tags
    }

    #[must_use]
    pub fn related(&self) -> &[RelatedPhoto] {
        &self.related
    }
}

impl DriveClient {
    pub async fn albums(&self, volume: &VolumeId) -> Result<Vec<AlbumSummary>> {
        Ok(self
            .api()
            .get(format!("drive/photos/volumes/{volume}/albums"))
            .pages(anchored())
            .collect(|response: AlbumsResponse| response.anchor.page(response.albums))
            .await?)
    }

    pub async fn album_photos(&self, volume: &VolumeId, album: &LinkId) -> Result<Vec<AlbumPhoto>> {
        Ok(self
            .api()
            .get(format!(
                "drive/photos/volumes/{volume}/albums/{album}/children"
            ))
            .query(&[("Sort", "Captured"), ("Desc", "1")])
            .pages(anchored())
            .collect(|response: AlbumPhotosResponse| response.anchor.page(response.photos))
            .await?)
    }

    pub async fn create_album(&self, library: &PhotoLibrary, name: &str) -> Result<Folder> {
        let pgp = self.pgp();
        let share = library.share();
        let root = library.root();
        let signer = crypto::signer(self.keys(), share.address_id())?;
        let email = crypto::email(self.keys(), share.address_id())?;
        let root_key = root.keys().key().public_key();

        let new_key = crypto::generate_key(pgp, &root_key, &signer)?;
        let armored_name = crypto::encrypt_name(pgp, &root_key, None, &signer, name)?;
        let (_, armored_hash_key) = crypto::generate_hash_key(pgp, &new_key.key)?;
        let request = CreateAlbumRequest {
            locked: false,
            link: NewAlbumLink {
                name: &armored_name,
                hash: &root.hash(name)?,
                node_key: &new_key.armored_key,
                node_passphrase: &new_key.armored_passphrase,
                node_passphrase_signature: &new_key.armored_signature,
                signature_email: &email,
                node_hash_key: &armored_hash_key,
                extended_attributes: None,
            },
        };

        let created: LinkId = self
            .api()
            .post(format!(
                "drive/photos/volumes/{}/albums",
                library.volume_id()
            ))
            .json(&request)
            .send_field_at(&["Album", "Link", "LinkID"])
            .await?;

        self.node(share.volume_id(), &created, root)
            .await?
            .into_folder()
            .ok_or(Error::NotAFolder(created))
    }

    pub async fn rename_album(
        &self,
        library: &PhotoLibrary,
        album: &Folder,
        name: &str,
    ) -> Result<()> {
        let share = library.share();
        let root = library.root();
        let signer = crypto::signer(self.keys(), share.address_id())?;
        let email = crypto::email(self.keys(), share.address_id())?;
        let armored_name = crypto::encrypt_name(
            self.pgp(),
            &root.keys().key().public_key(),
            Some(album.keys().name_session_key()),
            &signer,
            name,
        )?;
        let hash = root.hash(name)?;

        self.update_album(
            library.volume_id(),
            album.id(),
            &UpdateAlbumRequest {
                cover_link_id: None,
                link: Some(AlbumLinkUpdate {
                    name: &armored_name,
                    hash: &hash,
                    original_hash: album.link().hash(),
                    name_signature_email: &email,
                }),
            },
        )
        .await
    }

    pub async fn set_album_cover(
        &self,
        volume: &VolumeId,
        album: &LinkId,
        cover: &LinkId,
    ) -> Result<()> {
        self.update_album(
            volume,
            album,
            &UpdateAlbumRequest {
                cover_link_id: Some(cover),
                link: None,
            },
        )
        .await
    }

    pub async fn delete_album(
        &self,
        volume: &VolumeId,
        album: &LinkId,
        with_photos: bool,
    ) -> Result<()> {
        let deleted = self
            .api()
            .delete(format!("drive/photos/volumes/{volume}/albums/{album}"))
            .query(&[(
                "DeleteAlbumPhotos",
                rikka_proton_api::flag::to_query_value(with_photos),
            )])
            .send_unit()
            .await;

        match deleted {
            Ok(()) => Ok(()),
            Err(error) => match api_errors::album_outside_timeline(&error) {
                Some(children) => Err(Error::AlbumOutsideTimeline(children)),
                None => Err(error.into()),
            },
        }
    }

    pub async fn remove_from_album(
        &self,
        volume: &VolumeId,
        album: &LinkId,
        photos: &[LinkId],
    ) -> Result<Vec<Outcome<LinkId>>> {
        let mut outcomes = Vec::with_capacity(photos.len());
        for batch in photos.chunks(REMOVALS_PER_REQUEST) {
            let response: Vec<Outcome<LinkId>> = self
                .api()
                .post(format!(
                    "drive/photos/volumes/{volume}/albums/{album}/remove-multiple"
                ))
                .json(&RemovePhotosRequest { link_ids: batch })
                .send_field_or_default("Responses")
                .await?;
            outcomes.extend(match_outcomes(batch.iter(), response));
        }

        Ok(outcomes)
    }

    async fn update_album(
        &self,
        volume: &VolumeId,
        album: &LinkId,
        request: &UpdateAlbumRequest<'_>,
    ) -> Result<()> {
        self.api()
            .put(format!("drive/photos/volumes/{volume}/albums/{album}"))
            .json(request)
            .send_unit()
            .await?;

        Ok(())
    }
}

pub(super) fn match_outcomes<'a>(
    request: impl Iterator<Item = &'a LinkId>,
    response: Vec<Outcome<LinkId>>,
) -> Vec<Outcome<LinkId>> {
    request
        .map(|id| {
            response
                .iter()
                .find(|outcome| outcome.id() == Some(id))
                .cloned()
                .unwrap_or_else(|| Outcome::succeeded(id.clone()))
        })
        .collect()
}

#[derive(Debug, Serialize)]
struct CreateAlbumRequest<'a> {
    #[serde(rename = "Locked")]
    locked: bool,
    #[serde(rename = "Link")]
    link: NewAlbumLink<'a>,
}

#[derive(Debug, Serialize)]
struct NewAlbumLink<'a> {
    #[serde(rename = "Name")]
    name: &'a str,
    #[serde(rename = "Hash")]
    hash: &'a str,
    #[serde(rename = "NodeKey")]
    node_key: &'a str,
    #[serde(rename = "NodePassphrase")]
    node_passphrase: &'a str,
    #[serde(rename = "NodePassphraseSignature")]
    node_passphrase_signature: &'a str,
    #[serde(rename = "SignatureEmail")]
    signature_email: &'a str,
    #[serde(rename = "NodeHashKey")]
    node_hash_key: &'a str,
    #[serde(rename = "XAttr")]
    extended_attributes: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct UpdateAlbumRequest<'a> {
    #[serde(rename = "CoverLinkID", skip_serializing_if = "Option::is_none")]
    cover_link_id: Option<&'a LinkId>,
    #[serde(rename = "Link", skip_serializing_if = "Option::is_none")]
    link: Option<AlbumLinkUpdate<'a>>,
}

#[derive(Debug, Serialize)]
struct AlbumLinkUpdate<'a> {
    #[serde(rename = "Name")]
    name: &'a str,
    #[serde(rename = "Hash")]
    hash: &'a str,
    #[serde(rename = "OriginalHash")]
    original_hash: Option<&'a str>,
    #[serde(rename = "NameSignatureEmail")]
    name_signature_email: &'a str,
}

#[derive(Debug, Serialize)]
struct RemovePhotosRequest<'a> {
    #[serde(rename = "LinkIDs")]
    link_ids: &'a [LinkId],
}

#[derive(Debug, Deserialize)]
struct AlbumsResponse {
    #[serde(rename = "Albums", default)]
    albums: Vec<AlbumSummary>,
    #[serde(flatten)]
    anchor: Anchor,
}

#[derive(Debug, Deserialize)]
struct AlbumPhotosResponse {
    #[serde(rename = "Photos", default)]
    photos: Vec<AlbumPhoto>,
    #[serde(flatten)]
    anchor: Anchor,
}
