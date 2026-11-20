use jiff::Timestamp;

use crate::identifier::LinkId;
use crate::photo_properties::PhotoTag;

#[must_use]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PhotoDetails {
    captured_at: Option<Timestamp>,
    main_photo: Option<LinkId>,
    tags: Vec<PhotoTag>,
}

impl PhotoDetails {
    pub fn new() -> PhotoDetails {
        PhotoDetails::default()
    }

    pub fn captured_at(mut self, at: Timestamp) -> Self {
        self.captured_at = Some(at);
        self
    }

    pub fn main_photo(mut self, link: impl Into<LinkId>) -> Self {
        self.main_photo = Some(link.into());
        self
    }

    pub fn tag(mut self, tag: PhotoTag) -> Self {
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
        self
    }

    pub fn tags(self, tags: impl IntoIterator<Item = PhotoTag>) -> Self {
        tags.into_iter().fold(self, PhotoDetails::tag)
    }

    pub(super) fn capture_time(&self) -> Option<Timestamp> {
        self.captured_at
    }

    pub(super) fn main_photo_id(&self) -> Option<&LinkId> {
        self.main_photo.as_ref()
    }

    pub(super) fn tag_list(&self) -> &[PhotoTag] {
        &self.tags
    }
}
