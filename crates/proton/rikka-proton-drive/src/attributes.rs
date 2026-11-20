use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::media_attributes::{CameraAttributes, Location, MediaAttributes};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExtendedAttributes {
    #[serde(rename = "Common", default, skip_serializing_if = "Common::is_empty")]
    common: Common,
    #[serde(rename = "Media", default, skip_serializing_if = "Option::is_none")]
    media: Option<MediaAttributes>,
    #[serde(rename = "Location", default, skip_serializing_if = "Option::is_none")]
    location: Option<Location>,
    #[serde(rename = "Camera", default, skip_serializing_if = "Option::is_none")]
    camera: Option<CameraAttributes>,
    #[serde(flatten, default)]
    extra: Map<String, Value>,
}

impl ExtendedAttributes {
    #[must_use]
    pub fn new() -> ExtendedAttributes {
        ExtendedAttributes::default()
    }

    #[must_use]
    pub fn with_media(mut self, media: MediaAttributes) -> ExtendedAttributes {
        self.media = Some(media);
        self
    }

    #[must_use]
    pub fn with_location(mut self, location: Location) -> ExtendedAttributes {
        self.location = Some(location);
        self
    }

    #[must_use]
    pub fn with_camera(mut self, camera: CameraAttributes) -> ExtendedAttributes {
        self.camera = Some(camera);
        self
    }

    #[must_use]
    pub fn media(&self) -> Option<&MediaAttributes> {
        self.media.as_ref()
    }

    #[must_use]
    pub fn location(&self) -> Option<&Location> {
        self.location.as_ref()
    }

    #[must_use]
    pub fn camera(&self) -> Option<&CameraAttributes> {
        self.camera.as_ref()
    }

    #[must_use]
    pub fn with_modified_at(mut self, at: Timestamp) -> ExtendedAttributes {
        self.common.modification_time = Some(at.to_string());
        self
    }

    #[must_use]
    pub fn with_size(mut self, size: u64) -> ExtendedAttributes {
        self.common.size = Some(size);
        self
    }

    #[must_use]
    pub fn with_block_sizes(mut self, sizes: impl IntoIterator<Item = u64>) -> ExtendedAttributes {
        self.common.block_sizes = sizes.into_iter().collect();
        self
    }

    #[must_use]
    pub fn with_sha1(mut self, digest: impl Into<String>) -> ExtendedAttributes {
        self.common.digests = Some(Digests {
            sha1: digest.into(),
        });
        self
    }

    #[must_use]
    pub fn modified_at(&self) -> Option<Timestamp> {
        self.common
            .modification_time
            .as_deref()
            .and_then(|value| value.parse().ok())
    }

    #[must_use]
    pub fn size(&self) -> Option<u64> {
        self.common.size
    }

    #[must_use]
    pub fn block_sizes(&self) -> &[u64] {
        &self.common.block_sizes
    }

    #[must_use]
    pub fn sha1(&self) -> Option<&str> {
        self.common
            .digests
            .as_ref()
            .map(|digests| digests.sha1.as_str())
    }

    #[must_use]
    pub fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }

    pub(crate) fn read(json: &[u8]) -> Result<ExtendedAttributes> {
        serde_json::from_slice(json).map_err(Error::Attributes)
    }

    pub(crate) fn write(&self) -> Result<String> {
        serde_json::to_string(self).map_err(Error::Attributes)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Common {
    #[serde(
        rename = "ModificationTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    modification_time: Option<String>,
    #[serde(rename = "Size", default, skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
    #[serde(rename = "BlockSizes", default, skip_serializing_if = "Vec::is_empty")]
    block_sizes: Vec<u64>,
    #[serde(rename = "Digests", default, skip_serializing_if = "Option::is_none")]
    digests: Option<Digests>,
}

impl Common {
    fn is_empty(&self) -> bool {
        self == &Common::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Digests {
    #[serde(rename = "SHA1")]
    sha1: String,
}
