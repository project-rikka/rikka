use std::path::Path;

use rikka_types::Color;

use crate::cache::SeedCache;
use crate::error::{Error, Result};
use crate::extract;
use crate::stamp::Stamp;
use crate::thumbnail;

pub fn seed(path: impl AsRef<Path>) -> Result<Color> {
    seed_with(&mut SeedCache::load(), path.as_ref())
}

fn seed_with(cache: &mut SeedCache, path: &Path) -> Result<Color> {
    let stamp = Stamp::from_path(path)?;
    if let Some(source) = cache.get(path, stamp) {
        return Ok(source);
    }

    let source =
        extract::source_color(&thumbnail::read_rgba8(path)?).ok_or_else(|| Error::Empty {
            path: path.to_path_buf(),
        })?;
    cache.insert(path, stamp, source);
    Ok(source)
}
