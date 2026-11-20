mod transfers;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::SystemTime;

use ade_git::{Branch, Change, Changed, Commit, Draft, Patch, Repo, RepoState, Status, Worktree};
use ade_watch::Watch;
use rikka_app::prelude::*;

use crate::project::QUIET;
use crate::project::act::given;
use crate::project::git::group::foremost;
use crate::project::git::lines::Read;
use crate::project::git::operation::GitOperation;
use crate::project::git::painted::Painted;
use crate::project::git::patch_request::PatchRequest;
use crate::project::git::showing::Showing;
use crate::project::git::standing::Standing;
use crate::project::git::subject::Subject;
use crate::project::git::transferring::Transferring;
use crate::project::handle::ProjectHandle;
use crate::project::state::Project;

#[derive(Debug)]
struct Shown {
    of: Subject,
    read: Option<Result<Read, Arc<str>>>,
    painted: Rc<Painted>,
}

pub(crate) struct Git {
    repo: Option<Repo>,
    handle: ProjectHandle,
    watch: Watch,
    status: Status,
    changed: Changed,
    branches: Vec<Branch>,
    worktrees: Vec<Worktree>,
    history: Arc<[Commit]>,
    recalled_at: Option<String>,
    standing: Standing,
    reading: bool,
    recalling: bool,
    stale: bool,
    showing: Showing,
    unticked: BTreeSet<PathBuf>,
    shown: Option<Shown>,
    summary: TextDocument,
    description: TextDocument,
    operation: Option<GitOperation>,
    remote: Option<String>,
    fetched: Option<SystemTime>,
    attempted: Option<SystemTime>,
    fetch_due: Option<SystemTime>,
    transferring: Option<Transferring>,
    error_message: Option<Arc<str>>,
    root: PathBuf,
}

impl Git {
    pub(crate) fn open(root: PathBuf, handle: &ProjectHandle) -> Git {
        let git = Git {
            repo: None,
            handle: handle.clone(),
            watch: Watch::disabled(),
            status: Status::default(),
            changed: Changed::default(),
            branches: Vec::new(),
            worktrees: Vec::new(),
            history: Arc::from([] as [Commit; 0]),
            recalled_at: None,
            standing: Standing::Asking,
            reading: false,
            recalling: false,
            stale: false,
            showing: Showing::default(),
            unticked: BTreeSet::new(),
            shown: None,
            summary: TextDocument::new(""),
            description: TextDocument::new(""),
            operation: None,
            remote: None,
            fetched: None,
            attempted: None,
            fetch_due: None,
            transferring: None,
            error_message: None,
            root: root.clone(),
        };

        let discovering = root.clone();
        spawn_blocking(
            move || match Repo::discover(&discovering) {
                Ok(repo) => Some(repo),
                Err(error) => {
                    tracing::debug!(%error, path = %discovering.display(), "not a git repository");
                    None
                }
            },
            landing(&root, Git::discovered),
        );
        git
    }

    pub(crate) fn repo(&self) -> Option<&Repo> {
        self.repo.as_ref()
    }

    pub(crate) fn standing(&self) -> Standing {
        self.standing
    }

    pub(crate) fn status(&self) -> &Status {
        &self.status
    }

    pub(crate) fn changed(&self) -> &Changed {
        &self.changed
    }

    pub(crate) fn branches(&self) -> &[Branch] {
        &self.branches
    }

    pub(crate) fn worktrees(&self) -> &[Worktree] {
        &self.worktrees
    }

    pub(crate) fn history(&self) -> &Arc<[Commit]> {
        &self.history
    }

    pub(crate) fn is_recalling(&self) -> bool {
        self.recalling && self.recalled_at.is_none()
    }

    pub(crate) fn showing(&self) -> Showing {
        self.showing
    }

    pub(crate) fn summary(&self) -> &TextDocument {
        &self.summary
    }

    pub(crate) fn description(&self) -> &TextDocument {
        &self.description
    }

    pub(crate) fn operation(&self) -> Option<GitOperation> {
        self.operation
    }

    pub(crate) fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub(crate) fn is_ticked(&self, path: &Path) -> bool {
        !self.unticked.contains(path)
    }

    pub(crate) fn ticked(&self) -> CheckState {
        let changes = self.status.changes();
        match changes
            .iter()
            .filter(|change| !self.is_ticked(change.path()))
            .count()
        {
            0 => CheckState::Checked,
            off if off == changes.len() => CheckState::Unchecked,
            _ => CheckState::Indeterminate,
        }
    }

    pub(crate) fn shown(&self) -> Option<(&Subject, Option<Result<&Patch, &str>>)> {
        let shown = self.shown.as_ref()?;
        let patch = shown.read.as_ref().map(|read| match read {
            Ok(read) => Ok(read.patch.as_ref()),
            Err(message) => Err(message.as_ref()),
        });
        Some((&shown.of, patch))
    }

    pub(in crate::project) fn drawn(&self) -> Option<(&Read, &Rc<Painted>)> {
        let shown = self.shown.as_ref()?;
        let read = shown.read.as_ref()?.as_ref().ok()?;
        Some((read, &shown.painted))
    }

    pub(crate) fn landed(&self) -> Option<&Commit> {
        let Some((Subject::Commit(oid), _)) = self.shown() else {
            return None;
        };
        self.history.iter().find(|commit| commit.oid() == oid)
    }

    pub(crate) fn is_busy(&self) -> bool {
        self.operation.is_some()
            || self
                .transferring
                .is_some_and(|transferring| transferring.transfer().is_exclusive())
    }

    pub(crate) fn has_ticked(&self) -> bool {
        self.status
            .changes()
            .iter()
            .any(|change| self.is_ticked(change.path()))
    }

    pub(crate) fn is_committable(&self) -> bool {
        !self.is_busy() && !self.summary.text().trim().is_empty() && self.has_ticked()
    }

    pub(crate) fn tick(&mut self, path: PathBuf, ticked: bool) {
        match ticked {
            true => self.unticked.remove(&path),
            false => self.unticked.insert(path),
        };
    }

    pub(crate) fn reread(&mut self) {
        let Some(repo) = self.repo.clone() else {
            skip_view();
            return;
        };
        if self.reading {
            self.stale = true;
            skip_view();
            return;
        }
        self.reading = true;
        let root = self.root.clone();
        spawn_blocking(
            move || repo.state().map_err(|error| error.to_string()),
            given(
                move |project, read: Result<RepoState, String>| match project.workspace(&root) {
                    Some(workspace) => workspace.surveyed(read),
                    None => skip_view(),
                },
            ),
        );
    }

    fn discovered(&mut self, repo: Option<Repo>) {
        let Some(repo) = repo else {
            self.standing = Standing::Unversioned;
            return;
        };
        let root = self.root.clone();
        self.watch = Watch::new(
            QUIET,
            self.handle
                .sink(move |project, _| match project.workspace(&root) {
                    Some(workspace) => workspace.git_mut().reread(),
                    None => skip_view(),
                }),
        );
        self.repo = Some(repo);
        self.reread();
    }

    pub(crate) fn arrived(&mut self, read: Result<RepoState, String>) {
        self.reading = false;
        self.standing = Standing::Read;
        match read {
            Ok(survey) => {
                let RepoState {
                    status,
                    folders,
                    branches,
                    worktrees,
                    remote,
                    fetched,
                } = survey;
                self.status = status;
                self.branches = branches;
                self.worktrees = worktrees;
                self.remote = remote;
                self.fetched = fetched;
                self.watch.set_folders(folders);
                self.settle();
                self.schedule_fetch();
            }
            Err(message) => self.error_message = Some(Arc::from(message)),
        }

        if std::mem::take(&mut self.stale) {
            self.reread();
        }
        self.recall();
        self.reshow();
    }

    fn recall(&mut self) {
        let stale = self.recalled_at.as_deref() != self.status.head();
        let Some(repo) = self
            .repo
            .clone()
            .filter(|_| self.showing == Showing::History && stale && !self.recalling)
        else {
            return;
        };
        self.recalling = true;
        spawn_blocking(
            move || repo.history().map_err(|error| error.to_string()),
            landing(&self.root, Git::recalled),
        );
    }

    fn recalled(&mut self, recalled: Result<Vec<Commit>, String>) {
        self.recalling = false;
        match recalled {
            Ok(history) => {
                self.history = Arc::from(history);
                self.recalled_at = self.status.head().map(str::to_owned);
            }
            Err(message) => self.error_message = Some(Arc::from(message)),
        }
        self.reshow();
    }

    pub(crate) fn look(&mut self, showing: Showing) {
        if self.showing == showing {
            skip_view();
            return;
        }
        self.showing = showing;
        self.shown = None;
        self.recall();
        self.reshow();
    }

    fn settle(&mut self) {
        let changed: BTreeSet<&Path> = self.status.changes().iter().map(Change::path).collect();
        self.unticked
            .retain(|path| changed.contains(path.as_path()));

        self.changed = match self.repo.as_ref() {
            Some(repo) => Changed::new(repo.root(), self.status.changes()),
            None => Changed::default(),
        };
    }

    fn reshow(&mut self) {
        let looking = self.shown.as_ref().map(|shown| shown.of.clone());
        let next_subject = looking
            .filter(|subject| self.patch_request(subject).is_some())
            .or_else(|| self.first());

        match next_subject {
            Some(subject) => self.show(subject),
            None => self.shown = None,
        }
    }

    fn is_settled(&self, subject: &Subject) -> bool {
        matches!(subject, Subject::Commit(_))
            && self
                .shown
                .as_ref()
                .is_some_and(|shown| &shown.of == subject && matches!(shown.read, Some(Ok(_))))
    }

    fn patch_request(&self, subject: &Subject) -> Option<PatchRequest> {
        let repo = self.repo.clone()?;
        match subject {
            Subject::File(path) if self.showing == Showing::Changes => Some(PatchRequest::File {
                repo,
                change: self.change(path)?.clone(),
                born: self.status.is_born(),
            }),
            Subject::Commit(oid) if self.showing == Showing::History => {
                let commit = self.history.iter().find(|commit| commit.oid() == oid)?;
                Some(PatchRequest::Commit {
                    repo,
                    oid: commit.oid().to_owned(),
                })
            }
            _ => None,
        }
    }

    fn first(&self) -> Option<Subject> {
        match self.showing {
            Showing::Changes => foremost(self.status.changes())
                .map(|change| Subject::File(change.path().to_owned())),
            Showing::History => self
                .history
                .first()
                .map(|commit| Subject::Commit(commit.oid().to_owned())),
        }
    }

    pub(crate) fn show(&mut self, subject: Subject) {
        if self.is_settled(&subject) {
            return;
        }
        let Some(request) = self.patch_request(&subject) else {
            self.shown = None;
            return;
        };
        if self.shown.as_ref().is_none_or(|shown| shown.of != subject) {
            self.shown = Some(Shown {
                of: subject.clone(),
                read: None,
                painted: Rc::default(),
            });
        }
        let headed = matches!(subject, Subject::Commit(_));
        spawn_blocking(
            move || request.read().map(|patch| Read::from_patch(patch, headed)),
            landing(&self.root, move |git, read: Result<Read, String>| {
                git.showed(&subject, read);
            }),
        );
    }

    fn showed(&mut self, subject: &Subject, read: Result<Read, String>) {
        if self.shown.as_ref().is_none_or(|shown| &shown.of != subject) {
            return;
        }
        self.shown = Some(Shown {
            of: subject.clone(),
            read: Some(read.map_err(Arc::from)),
            painted: Rc::default(),
        });
    }

    pub(crate) fn tick_all(&mut self, ticked: bool) {
        self.unticked.clear();
        if !ticked {
            self.unticked.extend(
                self.status
                    .changes()
                    .iter()
                    .map(|change| change.path().to_owned()),
            );
        }
    }

    fn change(&self, path: &Path) -> Option<&Change> {
        self.status
            .changes()
            .iter()
            .find(|change| change.path() == path)
    }

    pub(crate) fn switch(&mut self, branch: &Branch) {
        let Some(repo) = self.repo.clone().filter(|_| !self.is_busy()) else {
            skip_view();
            return;
        };
        let branch = branch.clone();
        self.operation = Some(GitOperation::Switching);
        self.error_message = None;
        spawn_blocking(
            move || repo.switch(&branch).map_err(|error| error.to_string()),
            landing(&self.root, Git::switched),
        );
    }

    fn switched(&mut self, done: Result<(), String>) {
        self.operation = None;
        match done {
            Ok(()) => {
                self.shown = None;
                self.unticked.clear();
            }
            Err(message) => self.error_message = Some(Arc::from(message)),
        }
        self.reread();
    }

    pub(crate) fn commit(&mut self) {
        let Some(repo) = self.repo.clone().filter(|_| self.is_committable()) else {
            skip_view();
            return;
        };
        let draft = Draft::new(self.summary.text())
            .description(self.description.text())
            .with_changes(
                self.status
                    .changes()
                    .iter()
                    .map(|change| (change, self.is_ticked(change.path()))),
            );

        self.operation = Some(GitOperation::Committing);
        self.error_message = None;
        spawn_blocking(
            move || repo.commit(&draft).map_err(|error| error.to_string()),
            landing(&self.root, Git::committed),
        );
    }

    fn committed(&mut self, done: Result<(), String>) {
        self.operation = None;
        match done {
            Ok(()) => {
                self.summary = TextDocument::new("");
                self.description.reset("");
                self.unticked.clear();
            }
            Err(message) => self.error_message = Some(Arc::from(message)),
        }
        self.reread();
    }
}

fn landing<T: 'static>(
    root: &Path,
    land: impl Fn(&mut Git, T) + 'static,
) -> impl Fn(&mut Project, T) + 'static {
    let root = root.to_owned();
    given(move |project, outcome: T| match project.workspace(&root) {
        Some(workspace) => land(workspace.git_mut(), outcome),
        None => skip_view(),
    })
}

impl std::fmt::Debug for Git {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Git")
            .field("standing", &self.standing)
            .field("changes", &self.status.changes().len())
            .field("showing", &self.showing)
            .field("operation", &self.operation)
            .field("transferring", &self.transferring)
            .finish_non_exhaustive()
    }
}
