use rikka_media_format::{Extent, Format as Media};
use rikka_psd::FileHeader;

use crate::kind::Kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
    name: &'static str,
    kind: Kind,
    media: Option<Media>,
}

impl From<Media> for Format {
    fn from(media: Media) -> Self {
        Self {
            name: media.name(),
            kind: media.kind().into(),
            media: Some(media),
        }
    }
}

impl Format {
    pub(crate) const fn model(name: &'static str) -> Format {
        Format {
            name,
            kind: Kind::Model,
            media: None,
        }
    }

    pub(crate) const fn text(name: &'static str) -> Format {
        Format {
            name,
            kind: Kind::Text,
            media: None,
        }
    }

    pub(crate) const fn layered(name: &'static str) -> Format {
        Format {
            name,
            kind: Kind::Picture,
            media: None,
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        self.name
    }

    #[must_use]
    pub fn kind(self) -> Kind {
        self.kind
    }

    #[must_use]
    pub fn is_markdown(self) -> bool {
        self == crate::catalog::MARKDOWN
    }

    #[must_use]
    pub fn is_motion(self) -> bool {
        self == crate::catalog::VMD
    }

    #[must_use]
    pub fn is_photoshop(self) -> bool {
        self == crate::catalog::PSD || self == crate::catalog::PSB
    }

    pub(crate) fn extent(self, header: &[u8]) -> Option<Extent> {
        if self.is_photoshop() {
            let header = FileHeader::read(header).ok()?;
            return Extent::new(header.width, header.height);
        }
        self.media?.extent(header)
    }
}
