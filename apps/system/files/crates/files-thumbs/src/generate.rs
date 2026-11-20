use std::path::{Path, PathBuf};
use std::time::SystemTime;

use files_core::{Family, Location};
use rikka_image::{ColorScheme, Image};
use rikka_mime::MimeType;
use rikka_thumbnails::{Cache, Size};

use crate::external::Thumbnailers;

pub(crate) const APPLICATION: &str = "rikka-files";

const PIXEL_BUDGET: u64 = 100 * 1024 * 1024;
const DRAWING_SUBTYPES: [&str; 2] = ["svg+xml", "svg+xml-compressed"];
const STORED_SCHEME: ColorScheme = ColorScheme::Light;

pub(crate) fn make(
    cache: &Cache,
    thumbnailers: &Thumbnailers,
    at: &Location,
    mime: &MimeType,
    modified: SystemTime,
    size: Size,
) {
    let Some(original) = at.to_path() else {
        return;
    };
    let destination = cache.path_of(at.as_str(), size);
    if cache.prepare(&destination).is_err() {
        return;
    }
    let making = Generation {
        original,
        destination,
        at,
        modified,
        size,
    };

    let made = match Family::from_mime(mime) {
        Family::Image if DRAWING_SUBTYPES.contains(&mime.subtype()) => making.draw(cache),
        Family::Image => making.store(cache, &making.original),
        _ => making.generate_with(cache, thumbnailers, mime),
    };

    if !made {
        record_failure(cache, at, modified);
    }
}

struct Generation<'a> {
    original: PathBuf,
    destination: PathBuf,
    at: &'a Location,
    modified: SystemTime,
    size: Size,
}

impl Generation<'_> {
    fn generate_with(&self, cache: &Cache, thumbnailers: &Thumbnailers, mime: &MimeType) -> bool {
        let staging = self.staging();
        let made =
            thumbnailers.generate(mime, &self.original, self.at, &staging, self.size.pixels())
                && self.store(cache, &staging);
        std::fs::remove_file(&staging).ok();
        made
    }

    fn staging(&self) -> PathBuf {
        let name = self
            .destination
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.destination
            .with_file_name(format!(".{name}.{}.png", std::process::id()))
    }

    fn draw(&self, cache: &Cache) -> bool {
        let side = self.size.pixels();
        let drawn = rikka_image::read(&self.original)
            .color_scheme(STORED_SCHEME)
            .rasterize_svg_to_fit(side, side);
        drawn.is_ok_and(|drawn| self.keep(cache, drawn))
    }

    fn store(&self, cache: &Cache, picture: &Path) -> bool {
        let Ok(info) = rikka_image::read(picture).probe() else {
            return false;
        };
        if info.pixel_count() > PIXEL_BUDGET {
            return false;
        }
        let Ok(decoded) = rikka_image::read(picture).decode() else {
            return false;
        };
        let side = self.size.pixels();
        decoded
            .thumbnail(side, side)
            .is_ok_and(|shrunk| self.keep(cache, shrunk))
    }

    fn keep(&self, cache: &Cache, thumbnail: Image) -> bool {
        let bytes = std::fs::metadata(&self.original)
            .map(|meta| meta.len())
            .ok();
        cache
            .store(
                &self.destination,
                thumbnail,
                &Cache::fields(self.at.as_str(), self.modified, bytes),
            )
            .is_ok()
    }
}

fn record_failure(cache: &Cache, at: &Location, modified: SystemTime) {
    let marker = cache.failure_path(at.as_str(), APPLICATION);
    let Ok(blank) = Image::from_rgba8(1, 1, vec![0, 0, 0, 0]) else {
        return;
    };
    if let Err(error) = cache.store(&marker, blank, &Cache::fields(at.as_str(), modified, None)) {
        tracing::debug!("failed to record thumbnail failure: {error}");
    }
}
