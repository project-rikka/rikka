use std::io::ErrorKind;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

use rikka_backup::{Base, Bundle, File};
use rikka_fs::Replace;
use zeroize::Zeroizing;

use crate::dirs::Dirs;
use crate::error::{Error, Result};

const PRIVATE_DIR: u32 = 0o700;
const SEALED: &str = "sealed";
const CACHE: &str = "cache";
const SEALED_DEPTH: usize = 2;

#[derive(Clone, Copy, Debug)]
enum Keep {
    File,
    Sealed,
}

#[derive(Clone, Copy, Debug)]
struct Carried {
    base: Base,
    path: &'static str,
    keep: Keep,
}

const CARRIED: [Carried; 5] = [
    Carried {
        base: Base::State,
        path: "discord/accounts.json",
        keep: Keep::File,
    },
    Carried {
        base: Base::State,
        path: "music/accounts.json",
        keep: Keep::File,
    },
    Carried {
        base: Base::State,
        path: "proton",
        keep: Keep::Sealed,
    },
    Carried {
        base: Base::Config,
        path: "remote-viewer/hosts.json",
        keep: Keep::File,
    },
    Carried {
        base: Base::Config,
        path: "launcher/accounts.json",
        keep: Keep::File,
    },
];

pub(crate) fn collect(dirs: &Dirs) -> Result<Vec<File>> {
    let mut files = Vec::new();
    for carried in CARRIED {
        let root = dirs.of(carried.base);
        let relative = PathBuf::from(carried.path);
        match carried.keep {
            Keep::File => {
                if let Some(file) = read(root, carried.base, &relative)? {
                    files.push(file);
                }
            }
            Keep::Sealed => sealed(root, carried.base, &relative, 0, &mut files)?,
        }
    }
    Ok(files)
}

fn sealed(
    root: &Path,
    base: Base,
    relative: &Path,
    depth: usize,
    into: &mut Vec<File>,
) -> Result<()> {
    let folder = root.join(relative);
    let entries = match std::fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(Error::io(&folder)(error)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(Error::io(&folder))?;
        let kind = entry.file_type().map_err(Error::io(entry.path()))?;
        names.push((entry.file_name(), kind));
    }
    names.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (name, kind) in names {
        let child = relative.join(&name);
        if kind.is_dir() && depth < SEALED_DEPTH && name != CACHE {
            sealed(root, base, &child, depth + 1, into)?;
        } else if kind.is_file()
            && Path::new(&name)
                .extension()
                .is_some_and(|ext| ext == SEALED)
        {
            into.extend(read(root, base, &child)?);
        }
    }
    Ok(())
}

pub(crate) fn restore(bundle: &Bundle, dirs: &Dirs) -> Result<()> {
    for file in &bundle.files {
        let path = dirs.of(file.base()).join(file.path());
        if let Some(parent) = path.parent() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(PRIVATE_DIR)
                .create(parent)
                .map_err(Error::io(parent))?;
        }
        Replace::new(&path)
            .private()
            .write(file.contents())
            .map_err(Error::io(&path))?;
    }
    Ok(())
}

fn read(root: &Path, base: Base, relative: &Path) -> Result<Option<File>> {
    let path = root.join(relative);
    let contents = match std::fs::read(&path) {
        Ok(contents) => Zeroizing::new(contents),
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(Error::io(&path)(error)),
    };
    let relative = relative
        .to_str()
        .ok_or_else(|| Error::Unnamed(path.clone()))?;
    Ok(Some(File::new(base, relative, contents)?))
}
