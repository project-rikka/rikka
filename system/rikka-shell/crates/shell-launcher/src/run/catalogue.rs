use std::collections::BTreeSet;
use std::path::PathBuf;

use rikka_fuzzy::{Haystack, Score, best};
use rikka_open::is_executable;
use rikka_ui::MaterialSymbol;
use rikka_xdg::Sources;

use crate::provider::{Hit, Query};

#[derive(Debug, Default)]
pub(crate) struct Catalogue {
    programs: BTreeSet<String>,
    keys: Vec<Haystack>,
    sources: Sources,
}

impl Catalogue {
    #[must_use]
    pub(crate) fn reread(read: &Sources, directories: &[PathBuf]) -> Option<Catalogue> {
        if !read.is_stale(directories) {
            return None;
        }
        let mut programs = BTreeSet::new();
        for directory in directories {
            let Ok(listing) = std::fs::read_dir(directory) else {
                continue;
            };
            for entry in listing.flatten() {
                if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                    continue;
                }
                let path = entry.path();
                if !is_executable(&path) {
                    continue;
                }
                if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                    programs.insert(name.to_owned());
                }
            }
        }
        Some(Catalogue {
            keys: programs
                .iter()
                .map(|program| Haystack::new(program))
                .collect(),
            programs,
            sources: Sources::from_dirs(directories),
        })
    }

    pub(crate) fn sources(&self) -> &Sources {
        &self.sources
    }

    pub(super) fn knows(&self, program: &str) -> bool {
        self.programs.contains(program)
    }

    pub(crate) fn search(&self, query: &Query, limit: usize) -> Vec<Hit> {
        let matches = self
            .programs
            .iter()
            .zip(&self.keys)
            .filter_map(|(program, key)| {
                let score = query.find(key)?;
                (score > Score::NEUTRAL).then_some((score, program.as_str()))
            });
        best(matches, limit)
            .into_iter()
            .map(|(score, program)| {
                Hit::new(program, program)
                    .icon(MaterialSymbol::TERMINAL)
                    .score(score)
            })
            .collect()
    }
}
