use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_symbols::{BrandSymbol, IconGlyph, MaterialSymbol};

use crate::picture::ImageSource;

#[derive(Clone, Debug, PartialEq)]
pub enum IconSource {
    Glyph(IconGlyph),
    Picture(ImageSource),
    Mask(ImageSource),
}

impl IconSource {
    #[must_use]
    pub fn is_glyph(&self) -> bool {
        matches!(self, IconSource::Glyph(_))
    }
}

impl From<IconGlyph> for IconSource {
    fn from(glyph: IconGlyph) -> IconSource {
        IconSource::Glyph(glyph)
    }
}

impl From<MaterialSymbol> for IconSource {
    fn from(symbol: MaterialSymbol) -> IconSource {
        IconSource::Glyph(symbol.into())
    }
}

impl From<BrandSymbol> for IconSource {
    fn from(symbol: BrandSymbol) -> IconSource {
        IconSource::Glyph(symbol.into())
    }
}

impl From<ImageSource> for IconSource {
    fn from(picture: ImageSource) -> IconSource {
        IconSource::Picture(picture)
    }
}

impl From<Arc<Path>> for IconSource {
    fn from(path: Arc<Path>) -> IconSource {
        IconSource::Picture(ImageSource::file(path))
    }
}

impl From<&Path> for IconSource {
    fn from(path: &Path) -> IconSource {
        IconSource::Picture(ImageSource::from(path))
    }
}

impl From<PathBuf> for IconSource {
    fn from(path: PathBuf) -> IconSource {
        IconSource::Picture(ImageSource::from(path))
    }
}
