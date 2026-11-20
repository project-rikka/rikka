use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::read;

const MODULE_EXTENSION: &str = "slang";

pub(crate) fn taken(file: &Path) -> Result<Vec<PathBuf>> {
    let listed = read(file)?;
    std::fs::remove_file(file).map_err(|source| Error::Write {
        path: file.to_owned(),
        source,
    })?;
    let (_, sources) = listed
        .split_once(": ")
        .ok_or_else(|| Error::Dependencies(file.to_owned()))?;
    Ok(sources
        .replace("\\\n", " ")
        .split_whitespace()
        .map(PathBuf::from)
        .collect())
}

pub fn unused_modules(includes: &[PathBuf], sources: &BTreeSet<PathBuf>) -> Result<Vec<PathBuf>> {
    let Some(used) = sources
        .iter()
        .map(|source| std::fs::canonicalize(source).ok())
        .collect::<Option<BTreeSet<PathBuf>>>()
    else {
        return Ok(Vec::new());
    };
    let mut unused = Vec::new();
    for directory in includes.iter().filter(|directory| directory.is_dir()) {
        modules_under(directory, &mut |module| {
            let located = std::fs::canonicalize(&module).map_err(|source| Error::Read {
                path: module.clone(),
                source,
            })?;
            if !used.contains(&located) {
                unused.push(module);
            }
            Ok(())
        })?;
    }
    unused.sort();
    Ok(unused)
}

fn modules_under(directory: &Path, found: &mut impl FnMut(PathBuf) -> Result<()>) -> Result<()> {
    let failed = |source| Error::Read {
        path: directory.to_owned(),
        source,
    };
    for entry in std::fs::read_dir(directory).map_err(failed)? {
        let path = entry.map_err(failed)?.path();
        if path.is_dir() {
            modules_under(&path, found)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == MODULE_EXTENSION)
        {
            found(path)?;
        }
    }
    Ok(())
}
