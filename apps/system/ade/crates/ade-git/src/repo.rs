mod sync;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::branch::{Branch, Kind};
use crate::change::{Change, State};
use crate::commit::{self, Commit};
use crate::diff::Patch;
use crate::draft::Draft;
use crate::error::{Error, Result};
use crate::git::{self, EMPTY_TREE, git};
use crate::repo_state::RepoState;
use crate::stat::{self, Stat};
use crate::status::Status;
use crate::worktree::{Worktree, canonical_path};

const CONTEXT: &str = "-U3";

const HISTORY: usize = 200;

const FOLDERS: usize = 4_096;

const VERBATIM: [&str; 2] = ["--no-ext-diff", "--no-textconv"];

const PATHS: [&str; 2] = ["--pathspec-from-file=-", "--pathspec-file-nul"];

const WORKTREES_DIR: [&str; 2] = [".rikka", "worktrees"];

const INACTIVE_WORKTREE: [&str; 2] = ["bare", "prunable"];

const WORKTREES_EXCLUDE: &str = "/.rikka/worktrees/";

#[derive(Clone, Debug)]
pub struct Repo {
    root: PathBuf,
    git_dir: PathBuf,
    common_dir: PathBuf,
}

impl Repo {
    pub fn discover(at: &Path) -> Result<Repo> {
        let output = git(at)
            .args([
                "rev-parse",
                "--show-toplevel",
                "--absolute-git-dir",
                "--git-common-dir",
            ])
            .read()
            .map_err(|_| Error::Unversioned(at.to_owned()))?;

        let mut lines = output.lines().map(str::trim);
        let root = match lines.next().unwrap_or_default() {
            "" => return Err(Error::Unversioned(at.to_owned())),
            top_level => PathBuf::from(top_level),
        };
        let git_dir = lines
            .next()
            .map_or_else(|| root.join(".git"), |dir| root.join(dir));
        let common_dir = lines
            .next()
            .map_or_else(|| git_dir.clone(), |dir| root.join(dir));

        Ok(Repo {
            root,
            git_dir,
            common_dir,
        })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn state(&self) -> Result<RepoState> {
        let status = self.status()?;
        let worktrees = self.worktrees();
        Ok(RepoState {
            branches: self.branches(&status, &worktrees),
            folders: self.folders(&worktrees),
            remote: self.remote(status.branch()),
            fetched: self.fetched(),
            worktrees,
            status,
        })
    }

    pub fn status(&self) -> Result<Status> {
        let output = git(&self.root)
            .args([
                "status",
                "--porcelain=v2",
                "--branch",
                "--untracked-files=all",
                "-z",
            ])
            .read()?;
        let mut status = Status::parse(&output);
        let tracked = self.numstat(&status);
        let mut untracked = stat::Untracked::default();
        status.count(|change| match change.state() {
            State::Untracked => untracked.count(&self.root.join(change.path())),
            _ => tracked.get(change.path()).copied(),
        });
        Ok(status)
    }

    fn numstat(&self, status: &Status) -> BTreeMap<PathBuf, Stat> {
        let changes = status.changes();
        if changes
            .iter()
            .all(|change| change.state() == State::Untracked)
        {
            return BTreeMap::new();
        }
        let base = base(status.is_born());
        let mut stats = self.numstat_of(base, "--no-renames", Vec::new());
        let moved: Vec<PathBuf> = changes
            .iter()
            .filter(|change| change.state() == State::Renamed)
            .flat_map(Change::paths)
            .collect();
        if !moved.is_empty() {
            stats.extend(self.numstat_of(base, "--find-renames", moved));
        }
        stats
    }

    fn numstat_of(
        &self,
        base: &str,
        renames: &str,
        paths: Vec<PathBuf>,
    ) -> BTreeMap<PathBuf, Stat> {
        match git(&self.root)
            .args(["diff", "--numstat", "-z", renames])
            .args(VERBATIM)
            .args([base, "--"])
            .args(paths)
            .read()
        {
            Ok(output) => stat::numstat(&output),
            Err(error) => {
                tracing::debug!(%error, path = %self.root.display(), "failed to count changes");
                BTreeMap::new()
            }
        }
    }

    pub fn diff(&self, change: &Change, born: bool) -> Result<Patch> {
        match change.state() {
            State::Untracked => {
                let mut patch = Patch::parse(&self.untracked(change.path())?);
                patch.rename(change.path());
                Ok(patch)
            }
            State::Conflicted => Ok(Patch::parse(&self.against("--ours", change)?)),
            _ => Ok(Patch::parse(&self.against(base(born), change)?)),
        }
    }

    pub fn history(&self) -> Result<Vec<Commit>> {
        let output = git(&self.root)
            .args([
                "log",
                "--no-show-signature",
                &format!("--max-count={HISTORY}"),
                commit::FORMAT,
            ])
            .read()?;
        Ok(commit::read(&output))
    }

    pub fn patch(&self, oid: &str) -> Result<Patch> {
        let diff = git(&self.root)
            .args(["show", "--no-show-signature", "--format="])
            .args(VERBATIM)
            .args(["--find-renames", CONTEXT])
            .arg(oid)
            .read()?;
        Ok(Patch::parse(&diff))
    }

    pub fn at_head(&self, path: &Path) -> Result<String> {
        let inside = path.strip_prefix(&self.root).unwrap_or(path);
        git(&self.root)
            .arg("show")
            .arg(format!("HEAD:{}", inside.display()))
            .read()
    }

    pub fn commit(&self, draft: &Draft) -> Result<()> {
        if draft.summary().is_empty() {
            return Err(Error::Unsummarised);
        }
        let staged = draft.staged();
        if staged.is_empty() {
            return Err(Error::Nothing);
        }

        let unstaged = draft.unstaged();
        if !unstaged.is_empty() {
            self.unstage(unstaged)?;
        }
        let (standing, gone): (Vec<PathBuf>, Vec<PathBuf>) = staged
            .into_iter()
            .partition(|path| self.root.join(path).symlink_metadata().is_ok());
        self.stage(standing)?;
        self.untrack(gone)?;
        git(&self.root).arg("commit").args(draft.message()).run()
    }

    pub fn switch(&self, branch: &Branch) -> Result<()> {
        if branch.is_head() {
            return Ok(());
        }
        if branch.is_checked_out_elsewhere() {
            return Err(Error::BranchCheckedOut(branch.name().to_owned()));
        }
        let switching = git(&self.root).args(["switch", "--no-guess"]);
        let switching = match branch.kind() {
            Kind::Local => switching.arg(branch.name()),
            Kind::Remote => switching.args(["--track", branch.name()]),
        };
        switching.run()
    }

    #[must_use]
    pub fn branches(&self, status: &Status, worktrees: &[Worktree]) -> Vec<Branch> {
        let checked_out_elsewhere = |name: &str| {
            worktrees
                .iter()
                .any(|tree| !tree.is_here() && tree.branch() == Some(name))
        };
        let head = status.branch();

        let local = self.named(&["branch", "--list"]);
        let mut branches: Vec<Branch> = local
            .iter()
            .map(|name| {
                Branch::new(
                    name.clone(),
                    Kind::Local,
                    Some(name.as_str()) == head,
                    checked_out_elsewhere(name),
                )
            })
            .collect();

        branches.extend(
            self.named(&["branch", "--remotes", "--list"])
                .into_iter()
                .filter(|name| !name.contains("/HEAD"))
                .filter(|name| {
                    let leaf = name.split_once('/').map_or(name.as_str(), |(_, leaf)| leaf);
                    !local.iter().any(|here| here == leaf)
                })
                .map(|name| Branch::new(name, Kind::Remote, false, false)),
        );
        branches
    }

    #[must_use]
    pub fn worktrees(&self) -> Vec<Worktree> {
        let output = git(&self.root)
            .args(["worktree", "list", "--porcelain", "-z"])
            .read()
            .unwrap_or_default();
        worktrees(&output, &self.root)
    }

    pub fn add_worktree(&self, branch: &str, from: Option<&str>) -> Result<Worktree> {
        let branch = branch.trim();
        if branch.is_empty() {
            return Err(Error::Unnamed);
        }
        self.prune();

        let standing = self.worktrees();
        if standing.iter().any(|tree| tree.branch() == Some(branch)) {
            return Err(Error::BranchCheckedOut(branch.to_owned()));
        }

        let worktrees_dir = worktrees_dir(&standing, &self.root);
        create_folder(&worktrees_dir)?;
        self.exclude_worktrees()?;
        let at = unused_worktree_path(&worktrees_dir, branch);

        let adding = git(&self.root).args(["worktree", "add"]);
        let adding = match from {
            Some(base) => adding.args(["-b", branch]).arg(&at).arg(base),
            None => adding.arg(&at).arg(branch),
        };
        adding.run()?;

        Ok(Worktree::new(
            at,
            Some(branch.to_owned()),
            &canonical_path(&self.root),
            false,
        ))
    }

    fn prune(&self) {
        if let Err(error) = git(&self.root).args(["worktree", "prune"]).run() {
            tracing::debug!(%error, path = %self.root.display(), "failed to prune worktrees");
        }
    }

    fn exclude_worktrees(&self) -> Result<()> {
        let folder = self.common_dir.join("info");
        let at = folder.join("exclude");
        let existing = std::fs::read_to_string(&at).unwrap_or_default();
        if existing
            .lines()
            .any(|line| line.trim() == WORKTREES_EXCLUDE)
        {
            return Ok(());
        }

        let write_error = |error: std::io::Error| Error::Create {
            path: at.clone(),
            reason: error.to_string(),
        };
        let mut contents = existing;
        if !contents.is_empty() && !contents.ends_with('\n') {
            contents.push('\n');
        }
        contents.push_str(WORKTREES_EXCLUDE);
        contents.push('\n');

        std::fs::create_dir_all(&folder).map_err(write_error)?;
        std::fs::write(&at, contents).map_err(write_error)
    }

    #[must_use]
    pub fn folders(&self, worktrees: &[Worktree]) -> BTreeSet<PathBuf> {
        let mut folders = self.git_state_folders();
        folders.insert(self.root.clone());
        folders.insert(worktrees_dir(worktrees, &self.root));
        folders.extend(self.tracked_folders());
        folders
    }

    fn tracked_folders(&self) -> BTreeSet<PathBuf> {
        let output = git(&self.root)
            .args([
                "ls-files",
                "-z",
                "--cached",
                "--others",
                "--exclude-standard",
            ])
            .read()
            .unwrap_or_else(|error| {
                tracing::debug!(%error, path = %self.root.display(), "failed to list tracked folders");
                String::new()
            });

        let mut folders: BTreeSet<PathBuf> = git::fields(&output)
            .filter_map(|name| Path::new(name).parent())
            .map(|folder| self.root.join(folder))
            .collect();
        if folders.len() > FOLDERS {
            tracing::debug!(path = %self.root.display(), limit = FOLDERS, "too many tracked folders to watch");
            folders = folders.into_iter().take(FOLDERS).collect();
        }
        folders
    }

    fn git_state_folders(&self) -> BTreeSet<PathBuf> {
        let mut folders = BTreeSet::from([self.git_dir.clone(), self.common_dir.clone()]);
        folders.extend(nested(&self.common_dir.join("refs/heads")));
        folders.extend(nested(&self.common_dir.join("refs/remotes")));
        folders.insert(self.common_dir.join("logs"));
        folders.insert(self.common_dir.join("worktrees"));
        folders
    }

    fn named(&self, args: &[&str]) -> Vec<String> {
        git(&self.root)
            .args(args.iter().copied())
            .arg("--format=%(refname:short)")
            .read()
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect()
    }

    fn against(&self, base: &str, change: &Change) -> Result<String> {
        let mut diff = git(&self.root)
            .args(["diff"])
            .args(VERBATIM)
            .args(["--find-renames", CONTEXT])
            .arg(base)
            .arg("--");
        for path in change.paths() {
            diff = diff.arg(path);
        }
        diff.read()
    }

    fn untracked(&self, path: &Path) -> Result<String> {
        git(&self.root)
            .args(["diff"])
            .args(VERBATIM)
            .args([CONTEXT, "--no-index", "--"])
            .arg("/dev/null")
            .arg(self.root.join(path))
            .differs(true)
            .read()
    }

    fn stage(&self, paths: Vec<PathBuf>) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        git(&self.root)
            .args(["add", "--all"])
            .args(PATHS)
            .paths(paths)
            .run()
    }

    fn untrack(&self, paths: Vec<PathBuf>) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        git(&self.root)
            .args(["rm", "--cached", "--force", "--quiet", "--ignore-unmatch"])
            .args(PATHS)
            .paths(paths)
            .run()
    }

    fn unstage(&self, paths: Vec<PathBuf>) -> Result<()> {
        git(&self.root)
            .args(["reset", "--quiet"])
            .args(PATHS)
            .paths(paths)
            .run()
    }
}

fn base(born: bool) -> &'static str {
    match born {
        true => "HEAD",
        false => EMPTY_TREE,
    }
}

fn nested(at: &Path) -> BTreeSet<PathBuf> {
    let mut folders = BTreeSet::from([at.to_owned()]);
    let Ok(listing) = std::fs::read_dir(at) else {
        return folders;
    };
    folders.extend(listing.filter_map(|entry| {
        let entry = entry.ok()?;
        entry.file_type().ok()?.is_dir().then(|| entry.path())
    }));
    folders
}

fn worktrees(output: &str, here: &Path) -> Vec<Worktree> {
    let here = canonical_path(here);
    output
        .split("\0\0")
        .enumerate()
        .filter_map(|(at, record)| {
            let mut path = None;
            let mut branch = None;
            let mut checked_out = true;
            for line in record.split('\0').map(str::trim) {
                if let Some(worktree) = line.strip_prefix("worktree ") {
                    path = Some(PathBuf::from(worktree));
                } else if let Some(git_ref) = line.strip_prefix("branch ") {
                    branch = Some(
                        git_ref
                            .strip_prefix("refs/heads/")
                            .unwrap_or(git_ref)
                            .to_owned(),
                    );
                } else if INACTIVE_WORKTREE.contains(&first_word(line)) {
                    checked_out = false;
                }
            }
            let path = path?;
            if !checked_out || !path.is_dir() {
                return None;
            }
            Some(Worktree::new(path, branch, &here, at == 0))
        })
        .collect()
}

fn first_word(line: &str) -> &str {
    line.split_once(' ').map_or(line, |(first, _)| first)
}

fn worktrees_dir(standing: &[Worktree], here: &Path) -> PathBuf {
    let main = standing
        .iter()
        .find(|tree| tree.is_main())
        .map_or(here, Worktree::path);
    WORKTREES_DIR
        .iter()
        .fold(main.to_owned(), |at, leaf| at.join(leaf))
}

fn create_folder(folder: &Path) -> Result<()> {
    std::fs::create_dir_all(folder).map_err(|error| Error::Create {
        path: folder.to_owned(),
        reason: error.to_string(),
    })
}

fn unused_worktree_path(folder: &Path, branch: &str) -> PathBuf {
    let leaf: String = branch
        .chars()
        .map(
            |letter| match letter.is_alphanumeric() || letter == '-' || letter == '_' {
                true => letter,
                false => '-',
            },
        )
        .collect();

    let at = folder.join(&leaf);
    if !at.exists() {
        return at;
    }
    (2u32..)
        .map(|again| folder.join(format!("{leaf}-{again}")))
        .find(|again| !again.exists())
        .expect("u32 suffixes exhausted")
}
