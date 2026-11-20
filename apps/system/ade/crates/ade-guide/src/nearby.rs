use rikka_sync::Mutex;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rikka_tools::PathInstructions;

use crate::block::block;
use crate::guide::RIKKA;
use crate::reading;

#[derive(Debug)]
pub struct Nearby {
    root: PathBuf,
    handed: Mutex<BTreeSet<PathBuf>>,
}

impl PathInstructions for Nearby {
    fn instructions_for(&self, paths: &[PathBuf]) -> Option<String> {
        self.nearby(paths)
    }
}

impl Nearby {
    #[must_use]
    pub fn new(root: &Path) -> Nearby {
        Nearby {
            root: root.to_path_buf(),
            handed: Mutex::default(),
        }
    }

    #[must_use]
    fn nearby(&self, paths: &[PathBuf]) -> Option<String> {
        let mut handed = self.handed.lock();
        let mut blocks = Vec::new();

        for folder in self.between(paths) {
            let path = folder.join(RIKKA);
            if handed.contains(&path) {
                continue;
            }
            let Ok(Some(read)) = reading::read(&path) else {
                continue;
            };
            handed.insert(path.clone());
            blocks.push(block(&path, &read.text));
        }

        (!blocks.is_empty()).then(|| blocks.join("\n\n"))
    }

    pub fn forget(&self) {
        self.handed.lock().clear();
    }

    fn between(&self, paths: &[PathBuf]) -> BTreeSet<PathBuf> {
        let mut folders = BTreeSet::new();

        for path in paths {
            let mut folder = path.parent();
            while let Some(at) = folder {
                if at == self.root || !at.starts_with(&self.root) {
                    break;
                }
                folders.insert(at.to_path_buf());
                folder = at.parent();
            }
        }

        folders
    }
}
