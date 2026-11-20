use std::fs;
use std::path::Path;

use rikka_types::Lens;

use crate::colors::ThemeColors;
use crate::error::{Error, Result};
use crate::variant::Variant;

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct ThemeSpec {
    pub name: String,
    pub slug: String,
    pub variant: Variant,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default = "opaque")]
    pub opacity: f32,
    #[serde(default, deserialize_with = "crate::lens::table")]
    pub lens: Option<Lens>,
    pub colors: ThemeColors,
}

fn opaque() -> f32 {
    1.0
}

impl ThemeSpec {
    pub fn from_toml_str(source: &str) -> Result<Self> {
        let spec: Self = toml::from_str(source)?;
        if spec.slug.is_empty()
            || !spec
                .slug
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        {
            return Err(Error::InvalidSlug(spec.slug));
        }
        if !(0.0..=1.0).contains(&spec.opacity) {
            return Err(Error::InvalidOpacity(spec.opacity));
        }
        Ok(spec)
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let source = fs::read_to_string(path).map_err(|source| Error::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_toml_str(&source)
    }
}
