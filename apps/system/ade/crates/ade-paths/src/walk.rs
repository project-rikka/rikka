use std::ffi::OsStr;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use ignore::{DirEntry, ParallelVisitor, ParallelVisitorBuilder, WalkBuilder, WalkState};

pub(crate) const MOST: usize = 200_000;

const BATCH: usize = 1024;

const VCS: [&str; 6] = [".git", ".svn", ".hg", ".bzr", ".jj", ".sl"];

const SEALED: [&str; 3] = [".rikka/worktrees/", ".rikka/memory/", ".rikka/tmp/"];

pub(crate) fn walk(root: &Path) -> (Vec<String>, bool) {
    let results = Mutex::new(Vec::new());
    let visited = AtomicUsize::new(0);
    let mut walkers = Walkers {
        results: &results,
        visited: &visited,
        root,
    };

    WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .follow_links(false)
        .threads(threads())
        .build_parallel()
        .visit(&mut walkers);

    let mut paths = results.into_inner().unwrap_or_else(|poisoned| {
        tracing::warn!(path = %root.display(), "path walker panicked");
        poisoned.into_inner()
    });
    paths.sort_by(|one, other| {
        depth(one)
            .cmp(&depth(other))
            .then_with(|| one.as_str().cmp(other.as_str()))
    });
    let truncated = visited.load(Ordering::Relaxed) >= MOST;
    (paths, truncated)
}

fn depth(path: &str) -> usize {
    path.bytes().filter(|byte| *byte == b'/').count()
}

fn threads() -> usize {
    std::thread::available_parallelism()
        .map(|cores| cores.get().saturating_sub(2).clamp(1, 8))
        .unwrap_or(1)
}

struct Walkers<'a> {
    results: &'a Mutex<Vec<String>>,
    visited: &'a AtomicUsize,
    root: &'a Path,
}

impl<'a> ParallelVisitorBuilder<'a> for Walkers<'a> {
    fn build(&mut self) -> Box<dyn ParallelVisitor + 'a> {
        Box::new(Gathering {
            results: self.results,
            visited: self.visited,
            root: self.root,
            batch: Vec::with_capacity(BATCH),
        })
    }
}

struct Gathering<'a> {
    results: &'a Mutex<Vec<String>>,
    visited: &'a AtomicUsize,
    root: &'a Path,
    batch: Vec<String>,
}

impl Gathering<'_> {
    fn hand_over(&mut self) {
        if self.batch.is_empty() {
            return;
        }
        let batch = std::mem::take(&mut self.batch);
        if let Ok(mut results) = self.results.lock() {
            results.extend(batch);
        }
    }

    fn named(&self, entry: &DirEntry, folder: bool) -> Option<String> {
        let under = entry.path().strip_prefix(self.root).ok()?.to_str()?;
        Some(match folder {
            true => format!("{under}/"),
            false => under.to_owned(),
        })
    }
}

impl ParallelVisitor for Gathering<'_> {
    fn visit(&mut self, entry: Result<DirEntry, ignore::Error>) -> WalkState {
        let Ok(entry) = entry else {
            return WalkState::Continue;
        };
        if entry.depth() == 0 {
            return WalkState::Continue;
        }
        let folder = entry.file_type().is_some_and(|kind| kind.is_dir());
        if VCS.iter().any(|name| entry.file_name() == OsStr::new(name)) {
            return match folder {
                true => WalkState::Skip,
                false => WalkState::Continue,
            };
        }
        let Some(path) = self.named(&entry, folder) else {
            return WalkState::Continue;
        };
        if self.visited.fetch_add(1, Ordering::Relaxed) >= MOST {
            return WalkState::Quit;
        }
        let sealed = folder && SEALED.contains(&path.as_str());
        self.batch.push(path);
        if self.batch.len() >= BATCH {
            self.hand_over();
        }
        match sealed {
            true => WalkState::Skip,
            false => WalkState::Continue,
        }
    }
}

impl Drop for Gathering<'_> {
    fn drop(&mut self) {
        self.hand_over();
    }
}
