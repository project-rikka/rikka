use std::path::{Path, PathBuf};
use std::sync::Arc;

use files_core::Location;
use files_vfs::Vfs;
use rikka_mime::Database;

pub(crate) struct Sandbox(pub(crate) PathBuf);

impl Sandbox {
    pub(crate) fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("files-vfs-op-{name}"));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("create directory");
        Self(path)
    }

    pub(crate) fn dir(&self, relative: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(&path).expect("create directory");
        path
    }

    pub(crate) fn file(&self, relative: &str, contents: &[u8]) -> PathBuf {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create directory");
        }
        std::fs::write(&path, contents).expect("write file");
        path
    }

    pub(crate) fn at(&self, relative: &str) -> Location {
        Location::from_path(self.0.join(relative))
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

pub(crate) fn vfs() -> Arc<Vfs> {
    Arc::new(Vfs::new(Arc::new(Database::load())))
}

pub(crate) fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|_| panic!("{} missing", path.display()))
}
