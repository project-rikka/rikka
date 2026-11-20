use futures_util::{Stream, TryStreamExt as _, future};
use jiff::Timestamp;
use rikka_proton_api::{Page, Paging};
use serde::Deserialize;

use crate::client::DriveClient;
use crate::error::{Error, Result};
use crate::identifier::{LinkId, VolumeId};
use crate::photo_properties::PhotoTag;

const PHOTOS_PER_PAGE: u32 = 500;
const PREVIOUS_PAGE_LAST_LINK: &str = "PreviousPageLastLinkID";

#[derive(Debug, Clone, Deserialize)]
pub struct PhotoEntry {
    #[serde(rename = "LinkID")]
    link_id: LinkId,
    #[serde(
        rename = "CaptureTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    captured_at: Option<Timestamp>,
    #[serde(rename = "Hash", default)]
    hash: Option<String>,
    #[serde(rename = "ContentHash", default)]
    content_hash: Option<String>,
    #[serde(rename = "Tags", default)]
    tags: Vec<PhotoTag>,
    #[serde(rename = "RelatedPhotos", default)]
    related: Vec<RelatedPhoto>,
}

impl PhotoEntry {
    #[must_use]
    pub fn link_id(&self) -> &LinkId {
        &self.link_id
    }

    #[must_use]
    pub fn captured_at(&self) -> Option<Timestamp> {
        self.captured_at
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
    pub fn tags(&self) -> &[PhotoTag] {
        &self.tags
    }

    #[must_use]
    pub fn related(&self) -> &[RelatedPhoto] {
        &self.related
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RelatedPhoto {
    #[serde(rename = "LinkID")]
    link_id: LinkId,
    #[serde(
        rename = "CaptureTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    captured_at: Option<Timestamp>,
    #[serde(rename = "Hash", default)]
    hash: Option<String>,
    #[serde(rename = "ContentHash", default)]
    content_hash: Option<String>,
}

impl RelatedPhoto {
    #[must_use]
    pub fn link_id(&self) -> &LinkId {
        &self.link_id
    }

    #[must_use]
    pub fn captured_at(&self) -> Option<Timestamp> {
        self.captured_at
    }

    #[must_use]
    pub fn hash(&self) -> Option<&str> {
        self.hash.as_deref()
    }

    #[must_use]
    pub fn content_hash(&self) -> Option<&str> {
        self.content_hash.as_deref()
    }
}

impl DriveClient {
    pub fn timeline(&self, volume: &VolumeId) -> Timeline {
        Timeline {
            client: self.clone(),
            volume: volume.clone(),
            descending: true,
            page_size: PHOTOS_PER_PAGE,
            since: None,
            tag: None,
        }
    }
}

#[must_use]
#[derive(Debug, Clone)]
pub struct Timeline {
    client: DriveClient,
    volume: VolumeId,
    descending: bool,
    page_size: u32,
    since: Option<Timestamp>,
    tag: Option<PhotoTag>,
}

impl Timeline {
    pub fn oldest_first(mut self) -> Self {
        self.descending = false;
        self
    }

    pub fn page_size(mut self, size: u32) -> Self {
        self.page_size = size.max(1);
        self
    }

    pub fn captured_since(mut self, at: Timestamp) -> Self {
        self.since = Some(at);
        self
    }

    pub fn with_tag(mut self, tag: PhotoTag) -> Self {
        self.tag = Some(tag);
        self
    }

    pub fn pages(self) -> impl Stream<Item = Result<Vec<PhotoEntry>>> + Send + 'static {
        let mut query = vec![(
            "Desc",
            rikka_proton_api::flag::to_query_value(self.descending),
        )];
        if let Some(since) = self.since {
            query.push(("MinimumCaptureTime", since.as_second().to_string()));
        }
        if let Some(tag) = self.tag {
            query.push(("Tag", i32::from(tag).to_string()));
        }

        self.client
            .api()
            .get(format!("drive/volumes/{}/photos", self.volume))
            .query(&query)
            .pages(Paging::cursor(PREVIOUS_PAGE_LAST_LINK).page_size(self.page_size))
            .field("Photos")
            .stream(|photos: Vec<PhotoEntry>| {
                let next = photos.last().map(|last| last.link_id.clone());
                Page::new(photos).next(next)
            })
            .map_err(Error::from)
            .try_filter(|photos| future::ready(!photos.is_empty()))
    }

    pub async fn all(self) -> Result<Vec<PhotoEntry>> {
        self.pages().try_concat().await
    }
}
