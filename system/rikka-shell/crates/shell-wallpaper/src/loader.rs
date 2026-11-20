use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::SystemTime;

use rikka_sync::Mutex;
use rikka_ui::{VideoController, WeakVideoController};
use shell_kit::Placement;
use shell_kit::pictures::{self, Within};

use crate::media::{Media, Picture};

static DECODED: Mutex<Vec<(Stamp, CachedMedia)>> = Mutex::new(Vec::new());

#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    path: PathBuf,
    modified: Option<SystemTime>,
    len: u64,
    covered: Option<(u32, u32)>,
}

#[derive(Debug)]
enum CachedMedia {
    Still(Weak<Picture>),
    Video(WeakVideoController),
}

impl CachedMedia {
    fn media(&self) -> Option<Media> {
        match self {
            CachedMedia::Still(picture) => picture.upgrade().map(Media::Still),
            CachedMedia::Video(controller) => controller.upgrade().map(Media::Video),
        }
    }
}

pub fn load(path: &Path, covered: Option<(u32, u32)>) -> Result<Media, String> {
    let Some(stamp) = stamp(path, covered) else {
        return Err(format!("{} is not a file", path.display()));
    };

    {
        let mut decoded = DECODED.lock();
        if let Some(shared) = take_shared(&mut decoded, &stamp) {
            return Ok(shared);
        }
        if rikka_media_format::kind(path) != Some(rikka_media_format::Kind::Picture) {
            let controller = video(path);
            decoded.push((stamp, CachedMedia::Video(controller.downgrade())));
            return Ok(Media::Video(controller));
        }
    }

    let picture = Arc::new(decode(path, covered)?);
    let mut decoded = DECODED.lock();
    if let Some(shared) = take_shared(&mut decoded, &stamp) {
        return Ok(shared);
    }
    decoded.push((stamp, CachedMedia::Still(Arc::downgrade(&picture))));
    Ok(Media::Still(picture))
}

fn decode(path: &Path, covered: Option<(u32, u32)>) -> Result<Picture, String> {
    let developed = pictures::develop(path, covered, Within::Cover)?;
    Ok(Picture::new(developed.source()))
}

#[must_use]
pub fn shared(path: &Path, covered: Option<(u32, u32)>) -> Option<Media> {
    let stamp = stamp(path, covered)?;
    let mut decoded = DECODED.try_lock()?;
    take_shared(&mut decoded, &stamp)
}

fn video(path: &Path) -> VideoController {
    let controller = VideoController::open(path);
    controller.set_looping(true);
    controller.play();
    controller
}

pub fn cover_of(screen: &Placement) -> Option<(u32, u32)> {
    let side = |logical: f32| (logical * screen.scale).round() as u32;
    let (width, height) = (side(screen.size.width), side(screen.size.height));
    (width > 0 && height > 0).then_some((width, height))
}

fn stamp(path: &Path, covered: Option<(u32, u32)>) -> Option<Stamp> {
    let metadata = std::fs::metadata(path).ok()?;
    metadata.is_file().then(|| Stamp {
        path: path.to_path_buf(),
        modified: metadata.modified().ok(),
        len: metadata.len(),
        covered,
    })
}

fn take_shared(decoded: &mut Vec<(Stamp, CachedMedia)>, stamp: &Stamp) -> Option<Media> {
    let mut hit = None;
    decoded.retain(|(known, cached)| match cached.media() {
        Some(media) => {
            if known == stamp {
                hit = Some(media);
            }
            true
        }
        None => false,
    });
    hit
}
