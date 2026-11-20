use std::path::Path;

use rikka_fuzzy::{Haystack, Needle, best};

use crate::path_match::PathMatch;
use crate::walk;

#[derive(Debug, Default)]
pub struct Paths {
    paths: Vec<String>,
}

impl Paths {
    #[must_use]
    pub fn walk(root: &Path) -> Paths {
        let (paths, truncated) = walk::walk(root);
        if truncated {
            tracing::info!(path = %root.display(), limit = walk::MOST, "path listing truncated");
        }
        Paths { paths }
    }

    #[must_use]
    pub fn listed(paths: Vec<String>) -> Paths {
        Paths { paths }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    #[must_use]
    pub fn matching(&self, query: &str, most: usize) -> Vec<PathMatch> {
        let needle = Needle::with_smart_case(query);
        self.ranked(&needle, most, |_| true)
            .into_iter()
            .map(PathMatch::new)
            .collect()
    }

    #[must_use]
    pub fn files(&self, query: &str, most: usize) -> Vec<PathMatch> {
        let needle = Needle::with_smart_case(query);
        let mut positions = Vec::new();
        self.ranked(&needle, most, |path| !path.ends_with('/'))
            .into_iter()
            .map(|path| {
                let _ = needle.positions(&Haystack::new(path), &mut positions);
                PathMatch::picked(path, &positions)
            })
            .collect()
    }

    fn ranked(&self, needle: &Needle, most: usize, keep: impl Fn(&str) -> bool) -> Vec<&str> {
        let candidates = self
            .paths
            .iter()
            .map(String::as_str)
            .filter(|path| keep(path));
        if needle.is_empty() {
            return candidates.take(most).collect();
        }
        let scored = candidates.filter_map(|path| Some((needle.score(path)?, path)));
        best(scored, most)
            .into_iter()
            .map(|(_, path)| path)
            .collect()
    }
}
