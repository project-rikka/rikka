mod load;
mod patch;
mod resized;
mod save;
mod showing;
pub(crate) mod shown;

use std::sync::Arc;

use rikka_gpu::ImageFormat;

use crate::color::{Profile, Space};
use crate::error::Result;
use crate::histogram::Histogram;
use crate::imaging::{Formats, Imaging};
use crate::plane::Plane;
use crate::recipe::{Recipe, develop};

pub(crate) use load::upload;
pub use showing::Shown;
pub(crate) use shown::shown_parts;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Depth {
    Eight,
    Sixteen,
    Float,
}

impl Depth {
    pub(crate) fn storage(self, formats: Formats) -> ImageFormat {
        match self {
            Depth::Eight => ImageFormat::Rgba8,
            Depth::Sixteen => formats.deep,
            Depth::Float => ImageFormat::Rgba32Float,
        }
    }

    pub(crate) fn working(self) -> ImageFormat {
        match self {
            Depth::Eight => ImageFormat::Rgba16Float,
            Depth::Sixteen | Depth::Float => ImageFormat::Rgba32Float,
        }
    }

    pub(crate) fn mask(self, formats: Formats) -> ImageFormat {
        match self {
            Depth::Eight => formats.shallow_mask,
            Depth::Sixteen => formats.deep_mask,
            Depth::Float => ImageFormat::R32Float,
        }
    }

    pub(crate) fn clamps(self) -> bool {
        self != Depth::Float
    }

    pub(crate) fn steps(self) -> usize {
        match self {
            Depth::Eight => 4096,
            Depth::Sixteen | Depth::Float => 65536,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Rgb,
    Gray,
}

#[derive(Clone)]
pub struct Picture {
    inner: Arc<Inner>,
}

#[derive(Clone)]
pub(crate) struct Frame {
    pub(crate) plane: Plane,
    pub(crate) depth: Depth,
    pub(crate) mode: Mode,
    pub(crate) has_alpha: bool,
    pub(crate) space: Arc<Space>,
}

struct Inner {
    imaging: Imaging,
    frame: Frame,
}

impl Frame {
    pub(crate) fn extent(&self) -> [u32; 2] {
        self.plane.extent()
    }

    pub(crate) fn with_plane(&self, plane: Plane) -> Frame {
        Frame {
            plane,
            ..self.clone()
        }
    }
}

impl Picture {
    pub(crate) fn from_frame(imaging: &Imaging, frame: Frame) -> Picture {
        Picture {
            inner: Arc::new(Inner {
                imaging: imaging.clone(),
                frame,
            }),
        }
    }

    pub(crate) fn frame(&self) -> &Frame {
        &self.inner.frame
    }

    #[must_use]
    pub fn imaging(&self) -> &Imaging {
        &self.inner.imaging
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.inner.frame.plane.width()
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.inner.frame.plane.height()
    }

    #[must_use]
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width(), self.height())
    }

    #[must_use]
    pub fn depth(&self) -> Depth {
        self.inner.frame.depth
    }

    #[must_use]
    pub fn mode(&self) -> Mode {
        self.inner.frame.mode
    }

    #[must_use]
    pub fn has_alpha(&self) -> bool {
        self.inner.frame.has_alpha
    }

    #[must_use]
    pub fn profile(&self) -> Option<&Profile> {
        self.inner.frame.space.profile()
    }

    #[must_use]
    pub fn with_profile(&self, profile: Option<Profile>) -> Picture {
        let frame = Frame {
            space: self.inner.imaging.space(profile.as_ref()),
            ..self.inner.frame.clone()
        };
        Picture::from_frame(&self.inner.imaging, frame)
    }

    pub fn develop(&self, recipe: &Recipe) -> Result<Picture> {
        develop::develop(self, recipe)
    }

    pub fn histogram(&self) -> Result<Histogram> {
        Histogram::from_picture(self)
    }
}

impl PartialEq for Picture {
    fn eq(&self, other: &Picture) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl std::fmt::Debug for Picture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Picture")
            .field("width", &self.width())
            .field("height", &self.height())
            .field("depth", &self.depth())
            .field("mode", &self.mode())
            .field("alpha", &self.has_alpha())
            .field("profile", &self.profile())
            .finish()
    }
}
