use std::cell::Cell;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use music_core::{Artwork, Quality};
use rikka_app::prelude::*;
use rikka_dirs::Paths;
use rikka_net::DiskCache;
use rikka_ui::Observers;

const KEPT_FOR: Duration = Duration::from_secs(30 * 24 * 60 * 60);

#[derive(Default)]
struct Sharpness {
    quality: Cell<Quality>,
    observers: Observers,
}

thread_local! {
    static SHARPNESS: Sharpness = Sharpness::default();
}

pub(crate) fn set_quality(quality: Quality) {
    SHARPNESS.with(|sharpness| {
        if sharpness.quality.replace(quality) != quality {
            sharpness.observers.changed();
        }
    });
}

fn quality() -> Quality {
    SHARPNESS.with(|sharpness| {
        sharpness.observers.read();
        sharpness.quality.get()
    })
}

fn density(quality: Quality) -> f32 {
    match quality {
        Quality::Low => 1.0,
        Quality::Standard => 2.0,
        Quality::Highest => 4.0,
    }
}

pub(crate) fn cover(artwork: &Artwork, side: f32, radius: f32) -> Element {
    let artwork = artwork.clone();
    live_observed(move || framed(&artwork, side, radius))
        .size(Length::px(side), Length::px(side))
        .any()
}

fn framed(artwork: &Artwork, side: f32, radius: f32) -> Element {
    let pixels = (side * density(quality())).round() as u32;
    let picture = artwork
        .fitting(pixels)
        .and_then(|url| pictured(url.to_owned()).ready().cloned());
    let face = match picture {
        Some(bytes) => image(bytes)
            .fit(ImageFit::Cover)
            .radius(radius)
            .fill()
            .any(),
        None => icon(MaterialSymbol::MUSIC_NOTE)
            .extent(Length::px((side * 0.38).max(12.0)))
            .color(palette().text_muted)
            .any(),
    };
    container(face)
        .center()
        .background(palette().surface_variant)
        .radius(radius)
        .fill()
        .any()
}

fn pictured(url: String) -> Fetch<Arc<[u8]>> {
    fetch_blocking(("music-artwork", url.clone()), move || load(&url))
}

fn load(url: &str) -> rikka_net::Result<Arc<[u8]>> {
    if let Some(path) = rikka_open::uri_to_path(url) {
        return std::fs::read(path)
            .map(Arc::from)
            .map_err(rikka_net::Error::Io);
    }
    match disk_cache() {
        Some(disk_cache) => disk_cache.get_or_fetch(url),
        None => Err(rikka_net::Error::Io(std::io::Error::other(
            "the artwork cache is unavailable",
        ))),
    }
}

fn disk_cache() -> Option<&'static DiskCache> {
    static DISK_CACHE: OnceLock<Option<DiskCache>> = OnceLock::new();
    DISK_CACHE
        .get_or_init(|| {
            let folder = Paths::resolve()
                .ok()?
                .cache_dir()
                .join("music")
                .join("artwork");
            DiskCache::open(folder, KEPT_FOR)
                .inspect_err(|error| tracing::warn!("artwork cache unavailable: {error}"))
                .ok()
        })
        .as_ref()
}
