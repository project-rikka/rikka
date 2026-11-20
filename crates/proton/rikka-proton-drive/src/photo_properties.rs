use jiff::Timestamp;
use rikka_proton_api::wire_enum;
use serde::Deserialize;

use crate::identifier::LinkId;

wire_enum! {
    pub enum PhotoTag: default Favorites {
        Favorites = 0,
        Screenshots = 1,
        Videos = 2,
        LivePhotos = 3,
        MotionPhotos = 4,
        Selfies = 5,
        Portraits = 6,
        Bursts = 7,
        Panoramas = 8,
        Raw = 9,
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PhotoInfo {
    #[serde(
        rename = "CaptureTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    captured_at: Option<Timestamp>,
    #[serde(rename = "MainPhotoLinkID", default)]
    main_photo_id: Option<LinkId>,
    #[serde(rename = "Hash", default)]
    hash: Option<String>,
    #[serde(rename = "ContentHash", default)]
    content_hash: Option<String>,
    #[serde(rename = "RelatedPhotosLinkIDs", default)]
    related_ids: Vec<LinkId>,
}

impl PhotoInfo {
    #[must_use]
    pub fn captured_at(&self) -> Option<Timestamp> {
        self.captured_at
    }

    #[must_use]
    pub fn main_photo_id(&self) -> Option<&LinkId> {
        self.main_photo_id.as_ref()
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
    pub fn related_ids(&self) -> &[LinkId] {
        &self.related_ids
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PhotoProperties {
    #[serde(rename = "Albums", default)]
    albums: Vec<PhotoAlbum>,
    #[serde(rename = "Tags", default)]
    tags: Vec<PhotoTag>,
}

impl PhotoProperties {
    #[must_use]
    pub fn albums(&self) -> &[PhotoAlbum] {
        &self.albums
    }

    #[must_use]
    pub fn tags(&self) -> &[PhotoTag] {
        &self.tags
    }

    #[must_use]
    pub fn is_favorite(&self) -> bool {
        self.tags.contains(&PhotoTag::Favorites)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PhotoAlbum {
    #[serde(rename = "AlbumLinkID")]
    album_id: LinkId,
    #[serde(rename = "Hash", default)]
    hash: Option<String>,
    #[serde(rename = "ContentHash", default)]
    content_hash: Option<String>,
    #[serde(
        rename = "AddedTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    added_at: Option<Timestamp>,
}

impl PhotoAlbum {
    #[must_use]
    pub fn album_id(&self) -> &LinkId {
        &self.album_id
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
    pub fn added_at(&self) -> Option<Timestamp> {
        self.added_at
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AlbumProperties {
    #[serde(rename = "Locked", default)]
    locked: bool,
    #[serde(rename = "CoverLinkID", default)]
    cover_id: Option<LinkId>,
    #[serde(
        rename = "LastActivityTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    last_activity_at: Option<Timestamp>,
    #[serde(rename = "PhotoCount", default)]
    photo_count: u64,
    #[serde(rename = "NodeHashKey", default)]
    node_hash_key: Option<String>,
}

impl AlbumProperties {
    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    #[must_use]
    pub fn cover_id(&self) -> Option<&LinkId> {
        self.cover_id.as_ref()
    }

    #[must_use]
    pub fn last_activity_at(&self) -> Option<Timestamp> {
        self.last_activity_at
    }

    #[must_use]
    pub fn photo_count(&self) -> u64 {
        self.photo_count
    }

    #[must_use]
    pub fn node_hash_key(&self) -> Option<&str> {
        self.node_hash_key.as_deref()
    }
}
