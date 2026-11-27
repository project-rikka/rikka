use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rikka_account_book::{read_json, write_private, write_private_json};

use crate::error::Result;
use crate::file::File;
use crate::scope::Scope;

const LISTINGS: &str = "listings";
const OPENED: &str = "opened";
const THUMBNAILS: &str = "thumbnails";
const KINDS: [&str; 3] = ["png", "jpg", "gif"];
const UNNAMED: &str = "file";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shelf {
    root: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thumbnail {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
}

impl Shelf {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Shelf {
        Shelf { root: root.into() }
    }

    pub fn remember(&self, scope: &Scope, files: &[File]) -> Result<()> {
        let Some(name) = scope.cache_name() else {
            return Ok(());
        };
        let folder = self.root.join(LISTINGS);
        std::fs::create_dir_all(&folder).map_err(file_error(&folder))?;
        write_private_json(&folder.join(format!("{name}.json")), &files)?;
        Ok(())
    }

    pub fn recall(&self, scope: &Scope) -> Result<Option<Vec<File>>> {
        let Some(name) = scope.cache_name() else {
            return Ok(None);
        };
        let path = self.root.join(LISTINGS).join(format!("{name}.json"));
        Ok(read_json(&path)?)
    }

    #[must_use]
    pub fn opened(&self, file: &File, extension: Option<&str>) -> PathBuf {
        let mut name = rikka_format::safe_file_name(&file.name).unwrap_or_else(|| UNNAMED.into());
        if let Some(extension) = extension {
            name = format!("{name}.{extension}");
        }
        self.root.join(OPENED).join(revision(file)).join(name)
    }

    #[must_use]
    pub fn thumbnail(&self, file: &File) -> Option<Thumbnail> {
        let folder = self.root.join(THUMBNAILS);
        let path = KINDS
            .into_iter()
            .map(|extension| folder.join(format!("{}.{extension}", revision(file))))
            .find(|path| path.is_file())?;
        let (width, height, _) = measured(&std::fs::read(&path).ok()?)?;
        Some(Thumbnail {
            path,
            width,
            height,
        })
    }

    pub fn keep_thumbnail(&self, file: &File, picture: &[u8]) -> Result<Option<Thumbnail>> {
        let Some((width, height, extension)) = measured(picture) else {
            return Ok(None);
        };
        let folder = self.root.join(THUMBNAILS);
        std::fs::create_dir_all(&folder).map_err(file_error(&folder))?;
        let path = folder.join(format!("{}.{extension}", revision(file)));
        write_private(&path, picture)?;
        Ok(Some(Thumbnail {
            path,
            width,
            height,
        }))
    }

    pub fn prune_opened(&self, budget: u64) -> Result<()> {
        let folder = self.root.join(OPENED);
        let Ok(entries) = std::fs::read_dir(&folder) else {
            return Ok(());
        };
        let mut kept: Vec<(SystemTime, u64, PathBuf)> = entries
            .flatten()
            .map(|entry| entry.path())
            .map(|path| {
                let (used, bytes) = measure(&path);
                (used, bytes, path)
            })
            .collect();
        kept.sort_by_key(|(used, ..)| std::cmp::Reverse(*used));
        let mut total = 0_u64;
        for (_, bytes, path) in kept {
            total = total.saturating_add(bytes);
            if total > budget {
                std::fs::remove_dir_all(&path).map_err(file_error(&path))?;
            }
        }
        Ok(())
    }
}

fn revision(file: &File) -> String {
    let id: String = file
        .target_id()
        .chars()
        .filter(|letter| letter.is_ascii_alphanumeric() || matches!(letter, '-' | '_'))
        .collect();
    format!(
        "{id}-{}",
        file.version
            .or(file.modified_time.map(i64::unsigned_abs))
            .unwrap_or(0)
    )
}

fn measure(folder: &Path) -> (SystemTime, u64) {
    let mut used = SystemTime::UNIX_EPOCH;
    let mut bytes = 0;
    for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        bytes += metadata.len();
        let touched = metadata
            .accessed()
            .or_else(|_| metadata.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        used = used.max(touched);
    }
    (used, bytes)
}

fn file_error(path: &Path) -> impl FnOnce(std::io::Error) -> crate::error::Error + '_ {
    move |source| crate::error::Error::File {
        path: path.to_owned(),
        source,
    }
}

fn measured(picture: &[u8]) -> Option<(u32, u32, &'static str)> {
    let wide = |at: usize| {
        Some(u32::from(u16::from_be_bytes(
            picture.get(at..at + 2)?.try_into().ok()?,
        )))
    };
    if picture.starts_with(b"\x89PNG\r\n\x1a\n") {
        let side = |at: usize| {
            Some(u32::from_be_bytes(
                picture.get(at..at + 4)?.try_into().ok()?,
            ))
        };
        return Some((side(16)?, side(20)?, "png"));
    }
    if picture.starts_with(b"GIF8") {
        let side = |at: usize| {
            Some(u32::from(u16::from_le_bytes(
                picture.get(at..at + 2)?.try_into().ok()?,
            )))
        };
        return Some((side(6)?, side(8)?, "gif"));
    }
    if !picture.starts_with(b"\xff\xd8") {
        return None;
    }
    let mut at = 2;
    loop {
        let marker = *picture.get(at + 1)?;
        if *picture.get(at)? != 0xff {
            return None;
        }
        let frame = matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc);
        if frame {
            return Some((wide(at + 7)?, wide(at + 5)?, "jpg"));
        }
        at += 2 + usize::try_from(wide(at + 2)?).ok()?;
    }
}
