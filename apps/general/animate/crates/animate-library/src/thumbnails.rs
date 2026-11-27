use std::fs::Metadata;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rikka_image::{Image, Png};
use rikka_sync::Mutex;
use rikka_types::Choice;

use crate::details::{Details, Embedded};
use crate::entry::Entry;
use crate::error::{AtPath, Error, Result};
use crate::hash::Fnv;
use crate::probe;

const FOLDER: &str = "animate/thumbnails";
const EXTENSION: &str = "png";
const LARGEST_CACHE: u64 = 256 << 20;
const TRIMMED_CACHE: u64 = LARGEST_CACHE / 4 * 3;
const TOUCH_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const LARGEST_PICTURE: u32 = 8192;
const LARGEST_PICTURE_BYTES: u64 = 256 << 20;
const EMBEDDED: u64 = 0;
const STILL: u64 = 1;
const PERFORMED: u64 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FigureKey(u64);

impl FigureKey {
    pub fn from_file(path: &Path) -> io::Result<FigureKey> {
        let metadata = std::fs::metadata(path)?;
        Ok(FigureKey(
            stamped(Fnv::new().path(path), &metadata)?.finish(),
        ))
    }

    #[must_use]
    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Variant {
    Still,
    Performed(FigureKey),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Thumbnail {
    Ready(PathBuf),
    Pending,
    Extract,
    Render,
    Missing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ThumbnailSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl Choice for ThumbnailSize {
    const ALL: &'static [Self] = &[
        ThumbnailSize::Small,
        ThumbnailSize::Medium,
        ThumbnailSize::Large,
    ];

    fn slug(self) -> &'static str {
        match self {
            ThumbnailSize::Small => "small",
            ThumbnailSize::Medium => "medium",
            ThumbnailSize::Large => "large",
        }
    }
}

#[derive(Debug)]
pub struct Thumbnails {
    dir: PathBuf,
    usage: Mutex<Option<u64>>,
}

impl Thumbnails {
    pub const SIDE: u32 = 512;

    pub fn standard() -> Result<Thumbnails> {
        Ok(Thumbnails::new(
            rikka_dirs::Paths::resolve()?.cache_dir().join(FOLDER),
        ))
    }

    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Thumbnails {
        Thumbnails {
            dir: dir.into(),
            usage: Mutex::new(None),
        }
    }

    #[must_use]
    pub fn thumbnail(&self, entry: &Entry, variant: Variant) -> Thumbnail {
        if let Some(sidecar) = entry.sidecar() {
            return Thumbnail::Ready(sidecar.to_path_buf());
        }
        match entry.details() {
            Details::Pending => return Thumbnail::Pending,
            Details::Unreadable(_) => return Thumbnail::Missing,
            details if details.has_embedded_picture() => {
                return match self.fresh(entry, EMBEDDED, 0) {
                    Some(path) => Thumbnail::Ready(path),
                    None => Thumbnail::Extract,
                };
            }
            Details::Sound(_) => return Thumbnail::Missing,
            _ => {}
        }
        match self.cached(entry, variant) {
            Some(path) => Thumbnail::Ready(path),
            None => Thumbnail::Render,
        }
    }

    #[must_use]
    pub fn cached(&self, entry: &Entry, variant: Variant) -> Option<PathBuf> {
        let (kind, figure) = keyed(variant);
        self.fresh(entry, kind, figure)
    }

    pub fn store(
        &self,
        entry: &Entry,
        variant: Variant,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<PathBuf> {
        let image = Image::from_rgba8(width, height, rgba.to_vec())?;
        let (kind, figure) = keyed(variant);
        self.write(entry, kind, figure, &image)
    }

    pub fn extract(&self, entry: &Entry) -> Result<PathBuf> {
        let embedded = entry
            .details()
            .embedded()
            .ok_or(Error::Embedded("no embedded picture"))?;
        let bytes = match embedded {
            Embedded::Range { offset, length } => {
                probe::embedded_range(entry.path(), offset, length)
                    .map_err(|_| Error::Embedded("unreadable picture range"))?
            }
            Embedded::Cover => rikka_audio_tags::read(entry.path())?
                .tags
                .cover()
                .ok_or(Error::Embedded("no cover"))?
                .data
                .clone(),
        };
        let image = rikka_image::read(bytes.as_slice())
            .max_alloc(LARGEST_PICTURE_BYTES)
            .max_dimensions(LARGEST_PICTURE, LARGEST_PICTURE)
            .decode()?
            .thumbnail(Thumbnails::SIDE, Thumbnails::SIDE)?;
        self.write(entry, EMBEDDED, 0, &image)
    }

    fn fresh(&self, entry: &Entry, kind: u64, figure: u64) -> Option<PathBuf> {
        let path = self.path(entry, kind, figure)?;
        let metadata = std::fs::metadata(&path).ok()?;
        let touched = metadata.modified().ok()?;
        let now = SystemTime::now();
        if now.duration_since(touched).is_ok_and(|age| age > TOUCH_AGE)
            && let Err(error) = rikka_fs::set_modified(&path, now)
        {
            tracing::debug!(path = %path.display(), %error, "failed to touch thumbnail");
        }
        Some(path)
    }

    fn write(&self, entry: &Entry, kind: u64, figure: u64, image: &Image) -> Result<PathBuf> {
        let path = self
            .path(entry, kind, figure)
            .ok_or_else(|| Error::Undated(entry.path().to_path_buf()))?;
        let bytes = image.write(Png).to_bytes()?;
        rikka_fs::replace(&path, &bytes).at(&path)?;
        self.account(bytes.len() as u64);
        Ok(path)
    }

    fn path(&self, entry: &Entry, kind: u64, figure: u64) -> Option<PathBuf> {
        let modified = entry.modified().duration_since(UNIX_EPOCH).ok()?;
        let key = Fnv::new()
            .path(entry.path())
            .u64(entry.size())
            .u64(modified.as_secs())
            .u64(u64::from(modified.subsec_nanos()))
            .u64(kind)
            .u64(figure)
            .finish();
        Some(self.dir.join(format!("{key:016x}.{EXTENSION}")))
    }

    fn account(&self, added: u64) {
        let mut usage = self.usage.lock();
        let total = match *usage {
            Some(total) => total + added,
            None => cached_files(&self.dir)
                .iter()
                .map(|(_, metadata)| metadata.len())
                .sum(),
        };
        *usage = Some(match total > LARGEST_CACHE {
            true => trim(&self.dir),
            false => total,
        });
    }
}

fn keyed(variant: Variant) -> (u64, u64) {
    match variant {
        Variant::Still => (STILL, 0),
        Variant::Performed(FigureKey(figure)) => (PERFORMED, figure),
    }
}

fn stamped(hash: Fnv, metadata: &Metadata) -> io::Result<Fnv> {
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?;
    Ok(hash
        .u64(metadata.len())
        .u64(modified.as_secs())
        .u64(u64::from(modified.subsec_nanos())))
}

fn cached_files(dir: &Path) -> Vec<(PathBuf, Metadata)> {
    let Ok(listing) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    listing
        .filter_map(|child| {
            let path = child.ok()?.path();
            let thumbnail = path
                .extension()
                .is_some_and(|extension| extension == EXTENSION);
            let metadata = std::fs::metadata(&path)
                .ok()
                .filter(|metadata| metadata.is_file())?;
            thumbnail.then_some((path, metadata))
        })
        .collect()
}

fn trim(dir: &Path) -> u64 {
    let mut files = cached_files(dir);
    files.sort_by_key(|(_, metadata)| metadata.modified().unwrap_or(UNIX_EPOCH));
    let mut total: u64 = files.iter().map(|(_, metadata)| metadata.len()).sum();
    for (path, metadata) in files {
        if total <= TRIMMED_CACHE {
            break;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => total = total.saturating_sub(metadata.len()),
            Err(error) => {
                tracing::debug!(path = %path.display(), %error, "failed to evict thumbnail")
            }
        }
    }
    total
}
