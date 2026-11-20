use std::path::{Path, PathBuf};

use rikka_format::natural_cmp;

use crate::catalog;
use crate::kind::Kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Previous,
    Next,
}

#[derive(Clone, Debug)]
pub struct Neighbours {
    directory: PathBuf,
    names: Vec<String>,
    current: usize,
}

impl Neighbours {
    #[must_use]
    pub fn scan(path: &Path, kind: Kind) -> Self {
        let alone = || Self {
            directory: path.parent().unwrap_or(Path::new("")).to_path_buf(),
            names: vec![name_of(path)],
            current: 0,
        };
        let (Some(directory), Some(name)) = (path.parent(), path.file_name()) else {
            return alone();
        };
        let (Ok(listing), Some(name)) = (std::fs::read_dir(directory), name.to_str()) else {
            return alone();
        };

        let mut names: Vec<String> = listing
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|kind| !kind.is_dir()))
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|file_name| file_name == name || opens_as(file_name, kind))
            .collect();
        names.sort_by(|left, right| natural_cmp(left, right));

        let current = names
            .iter()
            .position(|file_name| file_name == name)
            .unwrap_or(0);
        Self {
            directory: directory.to_path_buf(),
            names,
            current,
        }
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.names.len()
    }

    #[must_use]
    pub fn index(&self) -> usize {
        self.current
    }

    #[must_use]
    pub fn step(&self, step: Step) -> Option<PathBuf> {
        let index = match step {
            Step::Previous => self.current.checked_sub(1)?,
            Step::Next => self.current + 1,
        };
        Some(self.directory.join(self.names.get(index)?))
    }

    pub fn move_to(&mut self, path: &Path) {
        let name = name_of(path);
        if let Some(current) = self.names.iter().position(|file_name| *file_name == name) {
            self.current = current;
        }
    }
}

fn opens_as(name: &str, kind: Kind) -> bool {
    !name.starts_with('.')
        && catalog::by_name(Path::new(name)).is_some_and(|format| format.kind() == kind)
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}
