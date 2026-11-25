use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha512};

use crate::config::ContentRecord;
use crate::error::{Error, Result};
use crate::folder::entries;
use crate::metadata::{LocalMetadata, read_metadata};

pub const DISABLED: &str = ".disabled";
const SAVES: &str = "saves";
const DATAPACKS: &str = "datapacks";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContentKind {
    Mod,
    ResourcePack,
    ShaderPack,
    DataPack,
}

impl ContentKind {
    pub const ALL: [ContentKind; 4] = [
        ContentKind::Mod,
        ContentKind::ResourcePack,
        ContentKind::ShaderPack,
        ContentKind::DataPack,
    ];

    #[must_use]
    pub const fn folder(self) -> &'static str {
        match self {
            ContentKind::Mod => "mods",
            ContentKind::ResourcePack => "resourcepacks",
            ContentKind::ShaderPack => "shaderpacks",
            ContentKind::DataPack => DATAPACKS,
        }
    }

    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            ContentKind::Mod => "jar",
            ContentKind::ResourcePack | ContentKind::ShaderPack | ContentKind::DataPack => "zip",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Place {
    Instance,
    World(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentFile {
    pub kind: ContentKind,
    pub place: Place,
    pub key: String,
    pub file_name: String,
    pub path: PathBuf,
    pub enabled: bool,
    pub record: ContentRecord,
    pub local: Option<LocalMetadata>,
}

impl ContentFile {
    #[must_use]
    pub fn title(&self) -> &str {
        self.local
            .as_ref()
            .and_then(|local| local.name.as_deref())
            .unwrap_or(&self.file_name)
    }
}

pub(crate) fn folder(root: &Path, kind: ContentKind, place: &Place) -> PathBuf {
    match place {
        Place::Instance => root.join(kind.folder()),
        Place::World(world) => root.join(SAVES).join(world).join(DATAPACKS),
    }
}

pub(crate) fn key(kind: ContentKind, place: &Place, file_name: &str) -> String {
    match place {
        Place::Instance => format!("{}/{file_name}", kind.folder()),
        Place::World(world) => format!("{SAVES}/{world}/{DATAPACKS}/{file_name}"),
    }
}

pub(crate) fn checked_name(name: &str) -> Result<&str> {
    let valid = !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', ':', '\0'])
        && !name.starts_with('.');
    match valid {
        true => Ok(name),
        false => Err(Error::FileName(name.to_owned())),
    }
}

fn modified(metadata: &fs::Metadata) -> i128 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |elapsed| elapsed.as_nanos() as i128)
}

fn digest(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).map_err(Error::io(path))?;
    let mut hasher = Sha512::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(Error::io(path))?;
        if read == 0 {
            return Ok(rikka_types::hex::encode(hasher.finalize()));
        }
        hasher.update(&buffer[..read]);
    }
}

pub(crate) fn record_for(path: &Path, cached: Option<&ContentRecord>) -> Result<ContentRecord> {
    let metadata = fs::metadata(path).map_err(Error::io(path))?;
    let size = metadata.len();
    let modified = modified(&metadata);
    if let Some(cached) = cached
        && cached.size == size
        && cached.modified == modified
        && !cached.sha512.is_empty()
    {
        return Ok(cached.clone());
    }
    let sha512 = digest(path)?;
    let unchanged = cached.is_some_and(|cached| cached.sha512 == sha512);
    Ok(ContentRecord {
        size,
        modified,
        sha512,
        identified: unchanged && cached.is_some_and(|cached| cached.identified),
        project_id: cached
            .filter(|_| unchanged)
            .and_then(|cached| cached.project_id.clone()),
        version_id: cached
            .filter(|_| unchanged)
            .and_then(|cached| cached.version_id.clone()),
        incompatible: unchanged && cached.is_some_and(|cached| cached.incompatible),
    })
}

pub(crate) fn scan(
    root: &Path,
    kind: ContentKind,
    place: &Place,
    cached: &std::collections::BTreeMap<String, ContentRecord>,
) -> Result<Vec<ContentFile>> {
    let folder = folder(root, kind, place);
    let mut files = Vec::new();
    for entry in entries(&folder)? {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let (file_name, enabled) = match name.strip_suffix(DISABLED) {
            Some(enabled) => (enabled, false),
            None => (name, true),
        };
        let matches = Path::new(file_name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case(kind.extension()));
        if !matches || !entry.file_type().map_err(Error::io(&path))?.is_file() {
            continue;
        }
        let key = key(kind, place, file_name);
        let record = record_for(&path, cached.get(&key))?;
        let local = read_metadata(kind, &path);
        files.push(ContentFile {
            kind,
            place: place.clone(),
            key,
            file_name: file_name.to_owned(),
            path,
            enabled,
            record,
            local,
        });
    }
    files.sort_by_key(|file| file.title().to_lowercase());
    Ok(files)
}
