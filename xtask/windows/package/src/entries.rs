use std::collections::BTreeMap;
use std::collections::btree_map::Entry as Slot;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Error, Result, bail};

const LIBRARY_EXTENSION: &str = ".dll";
const PROGRAM_EXTENSION: &str = ".exe";

#[derive(Clone, Debug)]
pub(crate) struct Placement {
    source: PathBuf,
    entry: String,
}

impl FromStr for Placement {
    type Err = Error;

    fn from_str(text: &str) -> Result<Placement> {
        let Some((source, entry)) = text.split_once('=') else {
            bail!("placement {text:?} is not SOURCE=ENTRY");
        };
        if !entry.is_empty() {
            check_entry(entry)?;
        }
        Ok(Placement {
            source: PathBuf::from(source),
            entry: entry.to_owned(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Executable,
    Data,
}

impl Mode {
    fn of(entry: &str) -> Mode {
        let lower = entry.to_ascii_lowercase();
        match lower.ends_with(PROGRAM_EXTENSION) || lower.ends_with(LIBRARY_EXTENSION) {
            true => Mode::Executable,
            false => Mode::Data,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) source: PathBuf,
    pub(crate) mode: Mode,
}

#[derive(Debug, Default)]
pub(crate) struct Entries(BTreeMap<String, Entry>);

impl Entries {
    pub(crate) fn program(&mut self, placement: &Placement) -> Result<()> {
        if !placement
            .entry
            .to_ascii_lowercase()
            .ends_with(PROGRAM_EXTENSION)
        {
            bail!("program entry {} is not an .exe", placement.entry);
        }
        if placement.entry.contains('/') {
            bail!(
                "program entry {} is not at the archive root",
                placement.entry
            );
        }
        self.file(placement)
    }

    pub(crate) fn file(&mut self, placement: &Placement) -> Result<()> {
        if placement.entry.is_empty() {
            bail!("{} has no entry name", placement.source.display());
        }
        let metadata = std::fs::metadata(&placement.source)
            .with_context(|| format!("failed to read {}", placement.source.display()))?;
        if !metadata.is_file() {
            bail!("{} is not a file", placement.source.display());
        }
        self.insert(placement.entry.clone(), placement.source.clone())
    }

    pub(crate) fn tree(&mut self, placement: &Placement) -> Result<()> {
        self.walk(&placement.source, &placement.entry)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Entry> {
        self.0.values()
    }

    pub(crate) fn binaries(&self) -> impl Iterator<Item = &Entry> {
        self.iter().filter(|entry| entry.mode == Mode::Executable)
    }

    pub(crate) fn shipped_libraries(&self) -> impl Iterator<Item = String> {
        self.iter().filter_map(|entry| {
            let lower = entry.name.to_ascii_lowercase();
            (!lower.contains('/') && lower.ends_with(LIBRARY_EXTENSION)).then_some(lower)
        })
    }

    fn walk(&mut self, directory: &Path, prefix: &str) -> Result<()> {
        let mut children = std::fs::read_dir(directory)
            .with_context(|| format!("failed to list {}", directory.display()))?
            .map(|child| child.map(|child| child.path()))
            .collect::<std::io::Result<Vec<_>>>()
            .with_context(|| format!("failed to list {}", directory.display()))?;
        children.sort();
        for child in children {
            let Some(name) = child.file_name().and_then(|name| name.to_str()) else {
                bail!("{} has a name that is not UTF-8", child.display());
            };
            let entry = match prefix.is_empty() {
                true => name.to_owned(),
                false => format!("{prefix}/{name}"),
            };
            check_entry(&entry)?;
            let metadata = std::fs::metadata(&child)
                .with_context(|| format!("failed to read {}", child.display()))?;
            if metadata.is_dir() {
                self.walk(&child, &entry)?;
            } else if metadata.is_file() {
                self.insert(entry, child)?;
            } else {
                bail!("{} is neither a file nor a directory", child.display());
            }
        }
        Ok(())
    }

    fn insert(&mut self, name: String, source: PathBuf) -> Result<()> {
        let folded = name.to_lowercase();
        match self.0.entry(folded) {
            Slot::Vacant(slot) => {
                let mode = Mode::of(&name);
                slot.insert(Entry { name, source, mode });
                Ok(())
            }
            Slot::Occupied(slot) => bail!(
                "{name} from {} clashes with {}",
                source.display(),
                slot.get().name
            ),
        }
    }
}

fn check_entry(entry: &str) -> Result<()> {
    let valid = !entry.starts_with('/')
        && !entry.ends_with('/')
        && !entry.contains('\\')
        && entry
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
    match valid {
        true => Ok(()),
        false => bail!("entry {entry:?} is not a relative path"),
    }
}
