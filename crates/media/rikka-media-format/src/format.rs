use std::fmt;
use std::path::Path;

use rikka_audio_decode::AudioFormat;

use crate::catalog;
use crate::extent::Extent;
use crate::kind::Kind;

pub(crate) type Extents = fn(&[u8]) -> Option<Extent>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    Picture(rikka_image::Format),
    Sound(Option<AudioFormat>),
    Movie,
}

impl Detail {
    #[must_use]
    pub const fn kind(self) -> Kind {
        match self {
            Detail::Picture(_) => Kind::Picture,
            Detail::Sound(_) => Kind::Sound,
            Detail::Movie => Kind::Movie,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Format {
    name: &'static str,
    detail: Detail,
    extent: Option<Extents>,
}

impl Format {
    pub(crate) const fn of_picture(format: rikka_image::Format) -> Format {
        Format::new(format.name(), Detail::Picture(format))
    }

    pub(crate) const fn of_sound(name: &'static str, hint: Option<AudioFormat>) -> Format {
        Format::new(name, Detail::Sound(hint))
    }

    pub(crate) const fn of_movie(name: &'static str) -> Format {
        Format::new(name, Detail::Movie)
    }

    const fn new(name: &'static str, detail: Detail) -> Format {
        Format {
            name,
            detail,
            extent: None,
        }
    }

    pub(crate) const fn measured(mut self, extent: Extents) -> Format {
        self.extent = Some(extent);
        self
    }

    #[must_use]
    pub fn from_path(path: impl AsRef<Path>) -> Option<Format> {
        catalog::by_name(path.as_ref())
    }

    #[must_use]
    pub fn identify(path: impl AsRef<Path>, header: &[u8]) -> Option<Format> {
        catalog::identify(path.as_ref(), header)
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn detail(self) -> Detail {
        self.detail
    }

    #[must_use]
    pub const fn kind(self) -> Kind {
        self.detail.kind()
    }

    #[must_use]
    pub fn extent(self, header: &[u8]) -> Option<Extent> {
        self.extent.and_then(|read| read(header))
    }
}

impl PartialEq for Format {
    fn eq(&self, other: &Format) -> bool {
        self.name == other.name
    }
}

impl Eq for Format {}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}
