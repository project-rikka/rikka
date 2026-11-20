use std::path::{Path, PathBuf};
use std::sync::Arc;

use ade_git::{Kind, Worktree};
use rikka_app::prelude::*;
use rikka_thread::Summary;

use crate::labels;
use crate::project::act::given_with_setup;
use crate::project::workspace::Workspace;
use crate::project::worktree::{self, Making, canonical_path};
use crate::setup::Setup;

use super::Project;

impl Project {
    pub(in crate::project) fn choosing<'a>(
        &self,
        offers: &'a [worktree::Offer],
    ) -> Option<worktree::Choosing<'a>> {
        if self.worktrees().is_empty() {
            return None;
        }

        let selected_root = self.selected_root();
        Some(worktree::Choosing {
            at: offers
                .iter()
                .position(|offer| offer.path() == selected_root),
            locked: self.summary().is_some_and(|summary| summary.turns() > 0),
            offers,
        })
    }

    pub(in crate::project) fn naming(&self) -> Option<worktree::Naming<'_>> {
        let making = self.making.as_ref()?;
        let bases = self.branches();
        Some(worktree::Naming {
            taken: bases
                .iter()
                .any(|branch| branch.kind() == Kind::Local && making.branch() == branch.name()),
            bases,
            making,
        })
    }

    pub(in crate::project) fn offers(&self) -> Vec<worktree::Offer> {
        self.worktrees()
            .iter()
            .map(|tree| worktree::Offer::new(self.kept_at(tree), worktree::named(tree)))
            .collect()
    }

    fn branches(&self) -> &[ade_git::Branch] {
        self.workspaces
            .get(&self.root)
            .map_or(&[], |workspace| workspace.git().branches())
    }

    fn worktrees(&self) -> &[Worktree] {
        self.workspaces
            .get(&self.root)
            .map_or(&[], |workspace| workspace.git().worktrees())
    }

    fn kept_at(&self, worktree: &Worktree) -> PathBuf {
        match worktree.canonical_path() == self.here {
            true => self.root.clone(),
            false => worktree.path().to_owned(),
        }
    }

    pub(super) fn named(&self, at: &Path) -> String {
        let canonical_path = canonical_path(at);
        self.worktrees()
            .iter()
            .find(|tree| tree.canonical_path() == canonical_path)
            .map(worktree::named)
            .unwrap_or_else(|| labels::file_name(at))
    }

    pub(in crate::project) fn shown(&self) -> &Workspace {
        let shown = self.shown_root();
        self.workspaces
            .get(&shown)
            .or_else(|| self.workspaces.get(&self.root))
            .expect("project root has no workspace")
    }

    pub(in crate::project) fn shown_root(&self) -> PathBuf {
        let selected_root = self.selected_root();
        match selected_root.is_dir() {
            true => selected_root,
            false => self.root.clone(),
        }
    }

    pub(super) fn selected_root(&self) -> PathBuf {
        let worktree = match self.summary() {
            Some(summary) => summary.worktree().map(Path::to_path_buf),
            None => (*self.pending).clone(),
        };
        worktree.unwrap_or_else(|| self.root.clone())
    }

    pub(in crate::project) fn pick_worktree(&mut self, root: PathBuf, setup: &Setup) {
        self.unmake();
        self.bind(root, setup);
    }

    pub(in crate::project) fn make_worktree(&mut self) {
        *self.making = Some(Making::new(self.head()));
    }

    pub(in crate::project) fn making(&mut self, act: impl FnOnce(&mut Making)) {
        if let Some(making) = self.making.as_mut() {
            act(making);
        }
    }

    pub(in crate::project) fn named_branch(&mut self) {
        if self.making.as_ref().is_some_and(Making::has_error) {
            self.making(Making::clear_error);
        }
    }

    pub(in crate::project) fn cancel_worktree(&mut self) {
        self.unmake();
    }
    fn head(&self) -> Option<String> {
        self.branches()
            .iter()
            .find(|branch| branch.is_head())
            .map(|branch| branch.name().to_owned())
    }

    fn bind(&mut self, root: PathBuf, setup: &Setup) {
        let worktree = (root != self.root).then(|| root.clone());
        match self.threads.summary(&self.open).map(Summary::turns) {
            Some(0) => {
                let open = self.open.to_string();
                let Some(thread_state) = self.thread_mut_of(&open) else {
                    skip_view();
                    return;
                };
                thread_state.set_worktree(worktree);
                self.keep(&open);
            }
            Some(_) => {
                skip_view();
                return;
            }
            None => self.pending.set(worktree),
        }
        self.enter(root, setup);
        let at = self.shown_root();
        self.terminals.follow(&self.open, &at);
    }

    pub(in crate::project) fn create_worktree(&mut self) {
        let Some(making) = self.making.as_ref() else {
            skip_view();
            return;
        };
        let branch = making.branch().to_owned();
        if branch.is_empty() || making.is_busy() {
            skip_view();
            return;
        }
        let base = making.base().map(str::to_owned);

        let branch_exists = self
            .branches()
            .iter()
            .any(|it| it.kind() == Kind::Local && it.name() == branch);
        let from = match branch_exists {
            true => None,
            false => Some(base.unwrap_or_else(|| "HEAD".to_owned())),
        };

        let repo = self
            .workspaces
            .get(&self.root)
            .and_then(|workspace| workspace.git().repo())
            .cloned();
        let Some(repo) = repo else {
            if let Some(making) = self.making.as_mut() {
                making.report_error(tr!("ade.git.unversioned").into());
            }
            return;
        };

        if let Some(making) = self.making.as_mut() {
            making.start();
        }
        spawn_blocking(
            move || {
                repo.add_worktree(&branch, from.as_deref())
                    .map_err(|error| error.to_string())
            },
            given_with_setup(|project, setup, made: Result<Worktree, String>| {
                project.created(made, setup);
            }),
        );
    }

    fn created(&mut self, made: Result<Worktree, String>, setup: &Setup) {
        let worktree = match made {
            Ok(worktree) => worktree,
            Err(message) => {
                if let Some(making) = self.making.as_mut() {
                    making.report_error(Arc::from(message));
                }
                return;
            }
        };

        self.unmake();
        let root = self.root.clone();
        if let Some(workspace) = self.workspaces.get_mut(&root) {
            workspace.git_mut().reread();
        }
        self.bind(worktree.path().to_owned(), setup);
    }
}
