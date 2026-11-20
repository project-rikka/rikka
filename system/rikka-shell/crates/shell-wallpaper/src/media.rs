use std::sync::Arc;

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use rikka_types::{Rect, Size};
use rikka_ui::{ImageSource, VideoController, VideoState};
use rikka_widgets::ImageFit;

#[derive(Clone, Debug)]
pub enum Media {
    Still(Arc<Picture>),
    Video(VideoController),
}

impl Media {
    pub(crate) fn is_still(&self) -> bool {
        matches!(self, Media::Still(_))
    }

    pub fn is_ready(&self) -> bool {
        match self {
            Media::Still(_) => true,
            Media::Video(controller) => controller.size().is_some(),
        }
    }

    pub(crate) fn failure(&self) -> Option<String> {
        match self {
            Media::Still(_) => None,
            Media::Video(controller) => (controller.state() == VideoState::Failed).then(|| {
                controller
                    .error()
                    .unwrap_or_else(|| "playback failed".into())
            }),
        }
    }

    pub fn changes(&self) -> BoxStream<'static, ()> {
        match self {
            Media::Still(_) => futures_util::stream::empty().boxed(),
            Media::Video(controller) => controller.changes().boxed(),
        }
    }

    pub fn picture(&self) -> Picture {
        match self {
            Media::Still(picture) => (**picture).clone(),
            Media::Video(controller) => Picture::new(ImageSource::Video(controller.clone())),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Picture {
    source: ImageSource,
}

impl Picture {
    pub(crate) fn new(source: ImageSource) -> Picture {
        Picture { source }
    }

    pub fn place(&self, bounds: Rect, fit: ImageFit) -> Rect {
        fit.place(bounds, self.source.natural_size().unwrap_or(Size::ZERO))
    }

    pub fn source(&self) -> ImageSource {
        self.source.clone()
    }
}
