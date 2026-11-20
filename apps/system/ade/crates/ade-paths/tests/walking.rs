use std::fs;

use ade_paths::{PathMatch, Paths};
use tempfile::TempDir;

fn project(files: &[&str]) -> TempDir {
    let root = TempDir::new().expect("a directory");
    for file in files {
        let path = root.path().join(file);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("a directory");
        }
        fs::write(path, b"").expect("a file");
    }
    root
}

fn paths(matches: &[PathMatch]) -> Vec<String> {
    matches.iter().map(|one| one.path().to_owned()).collect()
}

#[test]
fn walk_finds_files_and_folders() {
    let root = project(&["src/main.rs", "README.md"]);

    let walked = Paths::walk(root.path());

    let mut all = paths(&walked.matching("", 10));
    all.sort();
    assert_eq!(all, ["README.md", "src/", "src/main.rs"]);
}

#[test]
fn respects_gitignore() {
    let root = project(&["target/debug/build.rs", "src/main.rs"]);
    fs::write(root.path().join(".gitignore"), b"target/\n").expect("a file");

    let walked = Paths::walk(root.path());

    assert!(paths(&walked.matching("build", 10)).is_empty());
    assert_eq!(paths(&walked.matching("main", 10)), ["src/main.rs"]);
}

#[test]
fn skips_vcs_directories() {
    let root = project(&["src/main.rs", ".git/config", ".github/ci.yml"]);

    let walked = Paths::walk(root.path());
    let all = paths(&walked.matching("", 20));

    assert!(!all.iter().any(|path| path.starts_with(".git/")));
    assert!(all.iter().any(|path| path == ".github/"));
}

#[test]
fn rikka_folders_are_offered_as_folders() {
    let root = project(&[
        ".rikka/worktrees/feature/src/main.rs",
        ".rikka/memory/overview.md",
        ".rikka/tmp/build.log",
        ".rikka/skills/review/SKILL.md",
    ]);

    let walked = Paths::walk(root.path());
    let mut all = paths(&walked.matching("", 20));
    all.sort();

    assert_eq!(
        all,
        [
            ".rikka/",
            ".rikka/memory/",
            ".rikka/skills/",
            ".rikka/skills/review/",
            ".rikka/skills/review/SKILL.md",
            ".rikka/tmp/",
            ".rikka/worktrees/",
        ]
    );
}

#[test]
fn file_name_match_ranks_first() {
    let root = project(&["composer/one.rs", "src/composer.rs"]);

    let walked = Paths::walk(root.path());

    assert_eq!(
        paths(&walked.matching("composer.rs", 1)),
        ["src/composer.rs"]
    );
}
