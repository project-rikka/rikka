use std::path::{Path, PathBuf};
use std::process::Command;

use ade_git::Repo;

pub(crate) struct Linked {
    folder: tempfile::TempDir,
    pub(crate) repo: Repo,
    far: tempfile::TempDir,
}

impl Linked {
    pub(crate) fn root(&self) -> &Path {
        self.folder.path()
    }

    pub(crate) fn other(&self) -> PathBuf {
        self.far.path().join("other")
    }
}

pub(crate) fn repository() -> Option<(tempfile::TempDir, Repo)> {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let root = folder.path();

    git(root, &["init", "--quiet", "--initial-branch=main"])?;
    identify(root)?;

    write(root, ".gitignore", "target/\n");
    write(root, "src/main.rs", "fn main() {\n    let one = 1;\n}\n");
    git(root, &["add", "--all"])?;
    git(root, &["commit", "--quiet", "-m", "first"])?;

    let repo = Repo::discover(root).expect("the folder is a repository");
    Some((folder, repo))
}

pub(crate) fn linked() -> Option<Linked> {
    let (folder, repo) = repository()?;
    let far = tempfile::tempdir().expect("a temporary folder");
    let remote = far.path().join("remote.git");
    let remote = remote.to_str().expect("a temporary path is unicode");

    git(
        far.path(),
        &["init", "--quiet", "--bare", "--initial-branch=main", remote],
    )?;
    git(folder.path(), &["remote", "add", "origin", remote])?;
    git(
        folder.path(),
        &["push", "--quiet", "--set-upstream", "origin", "main"],
    )?;
    git(far.path(), &["clone", "--quiet", remote, "other"])?;
    identify(&far.path().join("other"))?;

    Some(Linked { folder, repo, far })
}

pub(crate) fn commit(root: &Path, path: &str, content: &str, message: &str) -> Option<()> {
    write(root, path, content);
    git(root, &["add", "--all"])?;
    git(root, &["commit", "--quiet", "-m", message])
}

fn identify(root: &Path) -> Option<()> {
    git(root, &["config", "user.name", "ADE"])?;
    git(root, &["config", "user.email", "ade@example.invalid"])?;
    git(root, &["config", "commit.gpgsign", "false"])
}

pub(crate) fn git(root: &Path, args: &[&str]) -> Option<()> {
    let ran = Command::new("git")
        .current_dir(root)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .ok()?;
    ran.status.success().then_some(())
}

pub(crate) fn write(root: &Path, path: &str, content: &str) {
    let at = root.join(path);
    if let Some(folder) = at.parent() {
        std::fs::create_dir_all(folder).expect("a folder to write into");
    }
    std::fs::write(at, content).expect("a file to write");
}

pub(crate) fn tracked(root: &Path) -> Vec<String> {
    let listed = Command::new("git")
        .current_dir(root)
        .args(["ls-tree", "-r", "--name-only", "HEAD"])
        .output()
        .expect("git says what the last commit holds");
    String::from_utf8_lossy(&listed.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}
