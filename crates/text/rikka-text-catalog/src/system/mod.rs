#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use std::path::{Path, PathBuf};

use crate::config::Fontconfig;

#[cfg(unix)]
pub(crate) use unix::{fallback_families, fonts, locale, preferred_families};
#[cfg(windows)]
pub(crate) use windows::{fallback_families, fonts, locale, preferred_families};

#[derive(Debug, Default)]
pub(crate) struct SystemFonts {
    pub(crate) fontconfig: Fontconfig,
    pub(crate) files: Vec<PathBuf>,
    pub(crate) embedded: Vec<EmbeddedFont>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EmbeddedFont {
    pub(crate) name: &'static str,
    pub(crate) bytes: &'static [u8],
}

impl EmbeddedFont {
    pub(crate) fn stem(&self) -> &str {
        Path::new(self.name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(self.name)
    }
}
