use std::io::ErrorKind;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

use rikka_backup::{Base, Bundle, File};
use rikka_fs::Replace;
use zeroize::Zeroizing;

use crate::dirs::Dirs;
use crate::error::{Error, Result};

const PRIVATE_DIR: u32 = 0o700;

#[derive(Clone, Copy, Debug)]
struct Carried {
    base: Base,
    path: &'static str,
}

const CARRIED: [Carried; 4] = [
    Carried {
        base: Base::State,
        path: "discord/accounts.json",
    },
    Carried {
        base: Base::State,
        path: "music/accounts.json",
    },
    Carried {
        base: Base::Config,
        path: "remote-viewer/hosts.json",
    },
    Carried {
        base: Base::Config,
        path: "launcher/accounts.json",
    },
];

pub(crate) fn collect(dirs: &Dirs) -> Result<Vec<File>> {
    let mut files = Vec::new();
    for carried in CARRIED {
        let root = dirs.of(carried.base);
        let relative = PathBuf::from(carried.path);
        if let Some(file) = read(root, carried.base, &relative)? {
            files.push(file);
        }
    }
    Ok(files)
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
