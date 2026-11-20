use std::path::{Path, PathBuf};

use libloading::os::unix::{Library, RTLD_GLOBAL, RTLD_NOW};

use crate::error::{Error, Result};

const RUNTIME: &str = "libstdc++.so.6";
const NEIGHBOUR: &str = "libgcc_s.so.1";

pub(crate) fn preload() -> Result<Library> {
    let direct = unsafe { Library::open(Some(RUNTIME), RTLD_NOW | RTLD_GLOBAL) };
    let first = match direct {
        Ok(library) => return Ok(library),
        Err(error) => error.to_string(),
    };
    let Some(folder) = loaded_folder(NEIGHBOUR) else {
        return Err(Error::Runtime(first));
    };
    let path = folder.join(RUNTIME);
    unsafe { Library::open(Some(&path), RTLD_NOW | RTLD_GLOBAL) }
        .map_err(|error| Error::Runtime(format!("{first}; {}: {error}", path.display())))
}

fn loaded_folder(library: &str) -> Option<PathBuf> {
    let maps = std::fs::read_to_string("/proc/self/maps").ok()?;
    maps.lines()
        .filter_map(|line| line.split_whitespace().nth(5))
        .map(Path::new)
        .find(|path| path.file_name().is_some_and(|name| name == library))
        .and_then(Path::parent)
        .map(Path::to_path_buf)
}
