use std::fs::{self, File, Metadata};
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use rikka_dirs::UserDirs;
use rikka_media_format::{Format, Kind};

use crate::error::{Error, Result};

const CHUNK: u64 = 256 * 1024;

#[derive(Clone, Debug)]
pub struct Library {
    folder: PathBuf,
}

impl Library {
    pub fn session() -> Result<Library> {
        let folder = UserDirs::load().wallpapers().ok_or(Error::NoPictures)?;
        rikka_dirs::ensure(&folder).map_err(Error::Folder)?;
        Ok(Library { folder })
    }

    #[must_use]
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    pub fn holding(&self, source: &Path) -> Result<Option<String>> {
        let wanted = fs::metadata(source).map_err(|error| read(source, error))?;
        let named = source.file_name().map(|name| self.folder.join(name));
        if let Some(named) = &named
            && let Some(id) = matching(source, &wanted, named)?
        {
            return Ok(Some(id));
        }
        let listing = fs::read_dir(&self.folder).map_err(|error| read(&self.folder, error))?;
        let mut candidates: Vec<PathBuf> = listing
            .flatten()
            .map(|entry| entry.path())
            .filter(|candidate| Some(candidate) != named.as_ref())
            .collect();
        candidates.sort();
        for candidate in candidates {
            if let Some(id) = matching(source, &wanted, &candidate)? {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    #[must_use]
    pub fn id_of(&self, placed: &Path) -> Option<String> {
        if placed.parent()? != self.folder {
            return None;
        }
        id(placed)
    }
}

#[must_use]
pub fn is_wallpaper(path: impl AsRef<Path>) -> bool {
    matches!(
        Format::from_path(path).map(Format::kind),
        Some(Kind::Picture | Kind::Movie)
    )
}

fn id(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    is_wallpaper(path).then(|| name.to_owned())
}

fn matching(source: &Path, wanted: &Metadata, candidate: &Path) -> Result<Option<String>> {
    let Some(id) = id(candidate) else {
        return Ok(None);
    };
    let found = match fs::metadata(candidate) {
        Ok(found) => found,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            skipped(candidate, &error);
            return Ok(None);
        }
    };
    if !found.is_file() || found.len() != wanted.len() {
        return Ok(None);
    }
    if (found.dev(), found.ino()) == (wanted.dev(), wanted.ino()) {
        return Ok(Some(id));
    }
    let other = match File::open(candidate) {
        Ok(other) => other,
        Err(error) => {
            skipped(candidate, &error);
            return Ok(None);
        }
    };
    let original = File::open(source).map_err(|error| read(source, error))?;
    Ok(same_contents(source, original, candidate, other)?.then_some(id))
}

fn same_contents(
    source: &Path,
    mut original: File,
    candidate: &Path,
    mut other: File,
) -> Result<bool> {
    let mut original_chunk = Vec::new();
    let mut other_chunk = Vec::new();
    loop {
        let filled =
            fill(&mut original, &mut original_chunk).map_err(|error| read(source, error))?;
        if let Err(error) = fill(&mut other, &mut other_chunk) {
            skipped(candidate, &error);
            return Ok(false);
        }
        if original_chunk != other_chunk {
            return Ok(false);
        }
        if filled == 0 {
            return Ok(true);
        }
    }
}

fn fill(file: &mut File, chunk: &mut Vec<u8>) -> io::Result<usize> {
    chunk.clear();
    file.by_ref().take(CHUNK).read_to_end(chunk)
}

fn skipped(candidate: &Path, error: &io::Error) {
    tracing::warn!(candidate = %candidate.display(), "skipped wallpaper candidate: {error}");
}

fn read(path: &Path, source: io::Error) -> Error {
    Error::Read {
        path: path.to_path_buf(),
        source,
    }
}
