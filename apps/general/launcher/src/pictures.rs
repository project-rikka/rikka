use std::sync::{Arc, OnceLock};
use std::time::Duration;

use rikka_app::prelude::*;
use rikka_net::DiskCache;

const KEPT_FOR: Duration = Duration::from_secs(30 * 24 * 60 * 60);

fn disk_cache() -> Option<&'static DiskCache> {
    static DISK_CACHE: OnceLock<Option<DiskCache>> = OnceLock::new();
    DISK_CACHE
        .get_or_init(|| {
            let folder = rikka_dirs::Paths::resolve()
                .ok()?
                .cache_dir()
                .join("launcher")
                .join("pictures");
            DiskCache::open(folder, KEPT_FOR)
                .inspect_err(|error| tracing::warn!("picture cache unavailable: {error}"))
                .ok()
        })
        .as_ref()
}

pub(crate) fn download(url: &str) -> rikka_net::Result<Arc<[u8]>> {
    match disk_cache() {
        Some(cache) => cache.get_or_fetch(url),
        None => Err(rikka_net::Error::Io(std::io::Error::other(
            "the picture cache is unavailable",
        ))),
    }
}

pub(crate) fn fetched(url: &str) -> Fetch<Arc<[u8]>> {
    let owned = url.to_owned();
    fetch_blocking(("launcher-picture", owned.clone()), move || {
        download(&owned)
    })
}

pub(crate) fn placeholder(symbol: MaterialSymbol, side: f32, radius: f32) -> Element {
    container(
        icon(symbol)
            .extent(Length::px((side * 0.46).max(14.0)))
            .color(palette().text_muted),
    )
    .center()
    .background(palette().surface_variant)
    .radius(radius)
    .size(Length::px(side), Length::px(side))
    .any()
}

pub(crate) fn framed(
    source: Option<ImageSource>,
    symbol: MaterialSymbol,
    side: f32,
    radius: f32,
) -> Element {
    match source {
        Some(source) => container(image(source).fit(ImageFit::Cover).radius(radius).fill())
            .background(palette().surface_variant)
            .radius(radius)
            .size(Length::px(side), Length::px(side))
            .any(),
        None => placeholder(symbol, side, radius),
    }
}

pub(crate) fn remote(url: Option<&str>, symbol: MaterialSymbol, side: f32, radius: f32) -> Element {
    let source = url
        .filter(|url| !url.is_empty())
        .and_then(|url| fetched(url).ready().cloned())
        .map(ImageSource::Bytes);
    framed(source, symbol, side, radius)
}
