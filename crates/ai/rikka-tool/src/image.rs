use std::sync::Arc;

use crate::media::Media;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    source: ImageSource,
    cached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageSource {
    Inline { media: Media, data: Arc<[u8]> },
    Url(String),
}

impl Image {
    #[must_use]
    pub fn png(data: impl Into<Arc<[u8]>>) -> Image {
        Image::inline(Media::Png, data)
    }

    #[must_use]
    pub fn jpeg(data: impl Into<Arc<[u8]>>) -> Image {
        Image::inline(Media::Jpeg, data)
    }

    #[must_use]
    pub fn gif(data: impl Into<Arc<[u8]>>) -> Image {
        Image::inline(Media::Gif, data)
    }

    #[must_use]
    pub fn webp(data: impl Into<Arc<[u8]>>) -> Image {
        Image::inline(Media::WebP, data)
    }

    #[must_use]
    pub fn inline(media: Media, data: impl Into<Arc<[u8]>>) -> Image {
        Image {
            source: ImageSource::Inline {
                media,
                data: data.into(),
            },
            cached: false,
        }
    }

    #[must_use]
    pub fn url(url: impl Into<String>) -> Image {
        Image {
            source: ImageSource::Url(url.into()),
            cached: false,
        }
    }

    #[must_use]
    pub fn cached(mut self) -> Self {
        self.cached = true;
        self
    }

    #[must_use]
    pub fn source(&self) -> &ImageSource {
        &self.source
    }

    #[must_use]
    pub fn is_cached(&self) -> bool {
        self.cached
    }
}
