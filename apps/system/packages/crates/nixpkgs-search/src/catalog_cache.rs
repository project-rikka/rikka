use std::cmp::Reverse;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use nixpkgs_catalog::Catalog;
use rikka_net::blocking::Client;

use crate::cancel::Cancel;
use crate::error::{Error, Result};
use crate::pin::{self, Pin};
use crate::release::{self, Release, Report};
use crate::stage::Stage;

const CATALOG_EXTENSION: &str = "catalog";
const DOWNLOAD_EXTENSION: &str = "json.br";

#[derive(Clone, Debug)]
pub struct CatalogCache {
    folder: PathBuf,
}

impl CatalogCache {
    #[must_use]
    pub fn new(folder: impl Into<PathBuf>) -> Self {
        Self {
            folder: folder.into(),
        }
    }

    pub fn open(
        &self,
        pin: &Pin,
        cancel: &Cancel,
        report: impl Fn(Stage) + Send + Sync + 'static,
    ) -> Result<Catalog> {
        let folder = self.folder.as_path();
        if let Some(commit) = pin.commit()
            && let Some(catalog) = load_cached(folder, |stem| pin::same_commit(stem, commit))
        {
            return Ok(catalog);
        }

        let report: Report = Arc::new(report);
        report(Stage::Locating);
        let downloaded = release::client().and_then(|client| {
            let release = Release::locate(&client, pin, cancel)?;
            let named = pin.commit().unwrap_or(release.commit());
            self.download(&client, &release, named, cancel, &report)
        });
        match downloaded {
            Err(Error::Cancelled) => Err(Error::Cancelled),
            Err(error) => load_cached(folder, |_| true).ok_or(error),
            catalog => catalog,
        }
    }

    fn download(
        &self,
        client: &Client,
        release: &Release,
        named: &str,
        cancel: &Cancel,
        report: &Report,
    ) -> Result<Catalog> {
        let catalog_path = self.folder.join(named).with_extension(CATALOG_EXTENSION);
        if let Ok(catalog) = Catalog::load(&catalog_path) {
            return Ok(catalog);
        }
        std::fs::create_dir_all(&self.folder).map_err(|error| nixpkgs_catalog::Error::Write {
            path: self.folder.clone(),
            source: error,
        })?;

        let download = catalog_path.with_extension(DOWNLOAD_EXTENSION);
        release.fetch(client, &download, cancel, report)?;
        report(Stage::Indexing);
        let catalog = Catalog::import(&download, release.name(), &catalog_path, &cancel.token())?;
        remove_stale(&self.folder, &catalog_path);
        Ok(catalog)
    }
}

fn load_cached(folder: &Path, accepts: impl Fn(&str) -> bool) -> Option<Catalog> {
    let mut cached: Vec<(SystemTime, PathBuf)> = std::fs::read_dir(folder)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == CATALOG_EXTENSION)
        })
        .filter(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(&accepts)
        })
        .map(|path| {
            let modified = path
                .metadata()
                .and_then(|meta| meta.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            (modified, path)
        })
        .collect();
    cached.sort_by_key(|(modified, _)| Reverse(*modified));
    cached
        .into_iter()
        .find_map(|(_, path)| Catalog::load(&path).ok())
}

fn remove_stale(folder: &Path, current: &Path) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        if path != current
            && path.is_file()
            && let Err(error) = std::fs::remove_file(&path)
        {
            tracing::warn!(%error, path = %path.display(), "failed to remove stale catalog");
        }
    }
}
