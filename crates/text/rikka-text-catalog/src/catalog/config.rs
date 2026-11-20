use std::path::PathBuf;

use crate::config::Environment;
use crate::locale::Locale;

const CACHE_SUBDIR: &str = "fonts";

#[derive(Clone, Debug, Default)]
pub(crate) enum Source {
    #[default]
    Listed,
    Fontconfig(PathBuf),
    System,
}

#[derive(Clone, Debug, Default)]
pub struct CatalogConfig {
    pub(crate) environment: Environment,
    pub(crate) source: Source,
    pub(crate) font_dirs: Vec<PathBuf>,
    pub(crate) cache_dir: Option<PathBuf>,
    pub(crate) locale: Option<Locale>,
}

impl CatalogConfig {
    pub fn from_env() -> Self {
        Self {
            environment: Environment::from_process(),
            source: Source::System,
            font_dirs: Vec::new(),
            cache_dir: rikka_dirs::Paths::resolve()
                .ok()
                .map(|paths| paths.cache_dir().join(CACHE_SUBDIR)),
            locale: Locale::from_env(),
        }
    }

    #[must_use]
    pub fn fontconfig(mut self, file: impl Into<PathBuf>) -> Self {
        self.source = Source::Fontconfig(file.into());
        self
    }

    #[must_use]
    pub fn font_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.font_dirs.push(dir.into());
        self
    }

    #[must_use]
    pub fn cache_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(dir.into());
        self
    }

    #[must_use]
    pub fn without_cache(mut self) -> Self {
        self.cache_dir = None;
        self
    }

    #[must_use]
    pub fn locale(mut self, locale: impl Into<Option<Locale>>) -> Self {
        self.locale = locale.into();
        self
    }
}
