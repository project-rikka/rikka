#[cfg(unix)]
use crate::drm::PlanarDmabuf;

use super::book::Kind;
use super::usage::Use;

pub(super) enum TextureSource {
    Memory,
    #[cfg(unix)]
    Dmabuf,
    #[cfg(unix)]
    Planar(PlanarDmabuf),
}

impl TextureSource {
    pub(super) fn is_imported(&self) -> bool {
        match self {
            Self::Memory => false,
            #[cfg(unix)]
            Self::Dmabuf | Self::Planar(_) => true,
        }
    }

    pub(super) fn kind(&self, usage: Use) -> Kind {
        match (self.is_imported(), usage) {
            (true, _) => Kind::Imported,
            (false, Use::Picture) => Kind::Picture,
            (false, Use::Canvas) => Kind::Canvas,
        }
    }
}
