use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rikka_i18n_catalog::{Catalog, Error};

#[derive(Debug)]
pub struct LoadError {
    pub path: PathBuf,
    pub error: Error,
}

#[derive(Debug, Default)]
pub struct LoadReport {
    pub loaded: Vec<Catalog>,
    pub errors: Vec<LoadError>,
}

impl LoadReport {
    pub fn warn(&self) {
        for LoadError { path, error } in &self.errors {
            tracing::warn!(path = %path.display(), %error, "skipping unloadable catalog");
        }
    }
}

pub fn load_dir(dir: impl AsRef<Path>) -> LoadReport {
    let dir = dir.as_ref();
    let mut report = LoadReport::default();

    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return report,
        Err(source) => {
            report.errors.push(LoadError {
                path: dir.to_path_buf(),
                error: Error::ReadFile {
                    path: dir.to_path_buf(),
                    source,
                },
            });
            return report;
        }
    };

    let mut paths = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => paths.push(entry.path()),
            Err(source) => report.errors.push(LoadError {
                path: dir.to_path_buf(),
                error: Error::ReadFile {
                    path: dir.to_path_buf(),
                    source,
                },
            }),
        }
    }
    paths.retain(|path| {
        path.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
    });
    paths.sort();

    for path in paths {
        match Catalog::from_path(&path) {
            Ok(catalog) => report.loaded.push(catalog),
            Err(error) => report.errors.push(LoadError { path, error }),
        }
    }
    report
}
