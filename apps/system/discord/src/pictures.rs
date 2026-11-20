use std::sync::{Arc, OnceLock};
use std::time::Duration;

use discord_core::Picture;
use rikka_app::prelude::*;
use rikka_dirs::Paths;
use rikka_net::DiskCache;

const KEPT_FOR: Duration = Duration::from_secs(14 * 24 * 60 * 60);
const DENSITY: f32 = 2.0;

pub(crate) fn fetched(picture: &Picture, side: f32) -> Option<Arc<[u8]>> {
    fetching(picture, side).ready().cloned()
}

pub(crate) fn fetching(picture: &Picture, side: f32) -> Fetch<Arc<[u8]>> {
    let url = url_at(picture, side);
    fetch_blocking(("discord-picture", url.clone()), move || bytes_of(&url))
}

pub(crate) fn load(picture: &Picture, side: f32) -> Result<Arc<[u8]>, rikka_net::Error> {
    bytes_of(&url_at(picture, side))
}

fn url_at(picture: &Picture, side: f32) -> String {
    picture.url_for_size((side * DENSITY).round() as u32)
}

fn bytes_of(url: &str) -> Result<Arc<[u8]>, rikka_net::Error> {
    match disk_cache() {
        Some(disk_cache) => disk_cache.get_or_fetch(url),
        None => Err(rikka_net::Error::Io(std::io::Error::other(
            "the picture cache is unavailable",
        ))),
    }
}

pub(crate) fn disk_cache_in(name: &str) -> Option<DiskCache> {
    let root = Paths::resolve()
        .map(|paths| paths.cache_dir().join("discord"))
        .unwrap_or_else(|_| std::env::temp_dir().join("rikka-discord"));
    DiskCache::open(root.join(name), KEPT_FOR)
        .inspect_err(|error| tracing::warn!("{name} cache unavailable: {error}"))
        .ok()
}

fn disk_cache() -> Option<&'static DiskCache> {
    static DISK_CACHE: OnceLock<Option<DiskCache>> = OnceLock::new();
    DISK_CACHE
        .get_or_init(|| disk_cache_in("pictures"))
        .as_ref()
}
