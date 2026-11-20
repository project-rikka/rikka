use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ade_command::Commands;
use ade_paths::Paths;
use ade_tree::{Entry, Tree};
use ade_watch::Watch;
use rikka_app::prelude::*;
use rikka_thread::Assistant;

use crate::project::QUIET;
use crate::project::act::given;
use crate::project::assembly::{self, Assembly};
use crate::project::catalogs::Catalogs;
use crate::project::explorer::Explorer;
use crate::project::file::Viewer;
use crate::project::git::Git;
use crate::project::handle::ProjectHandle;

const FRESH: Duration = Duration::from_secs(5);

pub(crate) struct Workspace {
    root: PathBuf,
    tree: Tree,
    git: Git,
    viewer: Viewer,
    explorer: Explorer,
    paths: Arc<Paths>,
    walked: Instant,
    walking: bool,
    catalogs: Catalogs,
    models: String,
    language: String,
    assistant: Option<Assistant>,
    stale: bool,
    built: u64,
}

impl Workspace {
    pub(crate) fn open(
        root: PathBuf,
        assembly: &Assembly,
        handle: &ProjectHandle,
    ) -> (Workspace, Option<Arc<str>>) {
        let git = Git::open(root.clone(), handle);
        let watching = root.clone();
        let watch = Watch::new(
            QUIET,
            handle.sink(move |project, folders| project.stirred(&watching, folders)),
        );

        let mut workspace = Workspace {
            tree: Tree::new(root.clone(), watch),
            git,
            viewer: Viewer::new(root.clone()),
            explorer: Explorer::default(),
            paths: Arc::new(Paths::default()),
            walked: Instant::now(),
            walking: true,
            catalogs: Catalogs::default(),
            models: String::new(),
            language: String::new(),
            assistant: None,
            stale: false,
            built: 0,
            root,
        };

        let error_message = workspace.build(assembly);
        read(workspace.root.clone(), workspace.root.clone());
        walk(workspace.root.clone());
        (workspace, error_message)
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn tree(&self) -> &Tree {
        &self.tree
    }

    pub(crate) fn git(&self) -> &Git {
        &self.git
    }

    pub(crate) fn git_mut(&mut self) -> &mut Git {
        &mut self.git
    }

    pub(crate) fn viewer(&self) -> &Viewer {
        &self.viewer
    }

    pub(crate) fn file(&mut self, act: impl FnOnce(&mut Viewer, &Git)) {
        act(&mut self.viewer, &self.git);
    }

    pub(crate) fn explorer(&self) -> &Explorer {
        &self.explorer
    }

    pub(crate) fn explore<R>(
        &mut self,
        act: impl FnOnce(&mut Explorer, WorkspaceParts<'_>) -> R,
    ) -> R {
        let parts = WorkspaceParts {
            root: &self.root,
            tree: &mut self.tree,
            viewer: &mut self.viewer,
            git: &self.git,
        };
        act(&mut self.explorer, parts)
    }

    pub(crate) fn paths(&self) -> Arc<Paths> {
        Arc::clone(&self.paths)
    }

    pub(crate) fn assistant(&self) -> Option<&Assistant> {
        self.assistant.as_ref()
    }

    pub(crate) fn built(&self) -> u64 {
        self.built
    }

    pub(crate) fn wither(&mut self) {
        self.stale = true;
    }

    pub(crate) fn opened(&mut self, folder: PathBuf, entries: Vec<Entry>) {
        self.tree.opened(folder, entries);
        let tree = &self.tree;
        self.explorer.keep(|path| tree.knows(path));
    }

    pub(crate) fn surveyed(&mut self, read: Result<ade_git::RepoState, String>) {
        self.git.arrived(read);
        self.viewer.rebase(&self.git);
    }

    pub(crate) fn settled(&mut self, paths: Arc<Paths>) {
        self.walking = false;
        self.walked = Instant::now();
        self.paths = paths;
    }

    pub(crate) fn is_stale(&self) -> bool {
        !self.walking && self.walked.elapsed() >= FRESH
    }

    pub(crate) fn freshen(&mut self) {
        if !self.is_stale() {
            return;
        }
        self.walking = true;
        walk(self.root.clone());
    }

    pub(crate) fn reconsider(&mut self, assembly: &Assembly) -> Option<Arc<str>> {
        let changed = self.models != assembly.models || self.language != assembly.language;
        if self.stale || changed {
            return self.build(assembly);
        }
        None
    }

    pub(crate) fn catalogs(&self) -> &Catalogs {
        &self.catalogs
    }

    pub(crate) fn recatalogue(
        &mut self,
        catalogs: Catalogs,
        assembly: &Assembly,
    ) -> Option<Arc<str>> {
        catalogs.report();
        self.catalogs = catalogs;
        self.build(assembly)
    }

    pub(crate) fn build(&mut self, assembly: &Assembly) -> Option<Arc<str>> {
        self.models = assembly.models.clone();
        self.language = assembly.language.clone();

        if let Some(previous) = self.assistant.take() {
            spawn_blocking(move || drop(previous), |(): ()| skip_view());
        }

        self.stale = false;
        match assembly::assemble(&self.root, assembly, &self.catalogs) {
            Ok(assistant) => {
                self.assistant = Some(assistant);
                self.built += 1;
                None
            }
            Err(error) => Some(tr!("ade.chat.unready", reason = &error.to_string()).into()),
        }
    }

    pub(crate) fn commands(&self) -> &Commands {
        &self.catalogs.commands
    }

    pub(crate) fn changed(&mut self, folders: BTreeSet<PathBuf>) {
        self.viewer.stirred(&folders, &self.git);

        for folder in folders.into_iter().filter(|folder| self.tree.holds(folder)) {
            read(self.root.clone(), folder);
        }
    }
}

pub(crate) struct WorkspaceParts<'a> {
    pub(crate) root: &'a Path,
    pub(crate) tree: &'a mut Tree,
    pub(crate) viewer: &'a mut Viewer,
    pub(crate) git: &'a Git,
}

impl WorkspaceParts<'_> {
    pub(crate) fn toggle(&mut self, folder: &Path) {
        if let Some(opened_folder) = self.tree.toggle(folder) {
            read(self.root.to_owned(), opened_folder);
        }
    }

    pub(crate) fn open_up(&mut self, folder: &Path) {
        if let Some(opened_folder) = self.tree.open_up(folder) {
            read(self.root.to_owned(), opened_folder);
        }
    }

    pub(crate) fn reveal(&mut self, path: &Path) {
        for folder in self.tree.reveal(path) {
            read(self.root.to_owned(), folder);
        }
    }

    pub(crate) fn refresh(&self, folders: impl IntoIterator<Item = PathBuf>) {
        let mut requested = BTreeSet::new();
        for folder in folders {
            if self.tree.holds(&folder) && requested.insert(folder.clone()) {
                read(self.root.to_owned(), folder);
            }
        }
    }
}

fn read(root: PathBuf, folder: PathBuf) {
    let reading = folder.clone();
    spawn_blocking(
        move || ade_tree::read(&reading),
        given(
            move |project, entries: Vec<Entry>| match project.workspace(&root) {
                Some(workspace) => workspace.opened(folder.clone(), entries),
                None => skip_view(),
            },
        ),
    );
}

fn walk(root: PathBuf) {
    let walking = root.clone();
    spawn_blocking(
        move || Arc::new(Paths::walk(&walking)),
        given(move |project, paths: Arc<Paths>| project.walked(&root, paths)),
    );
}

impl std::fmt::Debug for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workspace")
            .field("root", &self.root)
            .field("git", &self.git)
            .field("built", &self.built)
            .finish_non_exhaustive()
    }
}
