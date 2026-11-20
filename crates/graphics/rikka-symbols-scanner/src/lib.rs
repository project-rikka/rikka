mod coverage;
mod emit;
mod error;
mod listing;
mod symbolic;

use std::path::Path;

pub use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Material,
    Brands,
}

impl Kind {
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        match name {
            "material" => Some(Self::Material),
            "brands" => Some(Self::Brands),
            _ => None,
        }
    }

    fn type_name(self) -> &'static str {
        match self {
            Self::Material => "MaterialSymbol",
            Self::Brands => "BrandSymbol",
        }
    }
}

pub fn constants(kind: Kind, listing: &Path, font: &Path) -> Result<String> {
    let text = read(listing, |path| std::fs::read_to_string(path))?;
    let icons = match kind {
        Kind::Material => listing::material(listing, &text)?,
        Kind::Brands => listing::brands(listing, &text)?,
    };
    let bytes = read(font, |path| std::fs::read(path))?;
    coverage::check_coverage(font, &bytes, &icons)?;
    emit::constants(kind.type_name(), &icons)
}

pub fn symbolic_icon(listing: &Path, font: &Path, name: &str) -> Result<String> {
    let text = read(listing, |path| std::fs::read_to_string(path))?;
    let icons = listing::material(listing, &text)?;
    let bytes = read(font, |path| std::fs::read(path))?;
    let icon = icons
        .iter()
        .find(|icon| icon.name == name)
        .ok_or_else(|| Error::UnknownIcon {
            name: name.to_owned(),
            listing: listing.to_owned(),
        })?;
    symbolic::document(font, &bytes, icon)
}

fn read<T>(path: &Path, read_file: impl FnOnce(&Path) -> std::io::Result<T>) -> Result<T> {
    read_file(path).map_err(|source| Error::Read {
        path: path.to_owned(),
        source,
    })
}
