use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use rikka_audio_tags::{Audio, Tags};

use crate::artwork::Stored;
use music_core::fold;

use crate::hash::stable_hash;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Stat {
    pub(crate) path: PathBuf,
    pub(crate) device: i64,
    pub(crate) inode: i64,
    pub(crate) size: i64,
    pub(crate) modified: i64,
}

impl Stat {
    pub(crate) fn from_metadata(path: PathBuf, metadata: &Metadata) -> Stat {
        let modified = metadata
            .mtime()
            .saturating_mul(1_000_000_000)
            .saturating_add(metadata.mtime_nsec());
        Stat {
            path,
            device: signed(metadata.dev()),
            inode: signed(metadata.ino()),
            size: signed(metadata.size()),
            modified,
        }
    }

    pub(crate) fn same_stamp(&self, other: &Stat) -> bool {
        self.device == other.device
            && self.inode == other.inode
            && self.size == other.size
            && self.modified == other.modified
    }
}

fn signed(value: u64) -> i64 {
    i64::from_ne_bytes(value.to_ne_bytes())
}

#[derive(Clone, Debug)]
pub(crate) struct Record {
    pub(crate) stat: Stat,
    pub(crate) tags: Tags,
    pub(crate) audio: Audio,
    pub(crate) cover: Option<Stored>,
}

impl Record {
    pub(crate) fn title(&self) -> String {
        self.tags
            .title
            .clone()
            .unwrap_or_else(|| stem(&self.stat.path))
    }

    pub(crate) fn duration_ms(&self) -> i64 {
        i64::try_from(self.audio.duration.as_millis()).unwrap_or(i64::MAX)
    }

    pub(crate) fn print(&self) -> i64 {
        let title = fold(&self.title());
        let artists = fold(&self.tags.artists.join("\u{1f}"));
        let album = fold(self.tags.album.as_deref().unwrap_or_default());
        let track = self.tags.track.number.unwrap_or_default().to_string();
        let disc = self.tags.disc.number.unwrap_or_default().to_string();
        stable_hash([
            title.as_str(),
            artists.as_str(),
            album.as_str(),
            track.as_str(),
            disc.as_str(),
        ])
    }
}

pub(crate) fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}
