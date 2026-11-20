use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use files_core::Location;
use files_find::{Completion, Hit, Query, start};

struct Sandbox(std::path::PathBuf);

impl Sandbox {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("files-find-{name}"));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("create directory");
        Self(path)
    }

    fn file(&self, relative: &str, contents: &[u8]) {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create directory");
        }
        std::fs::write(path, contents).expect("write file");
    }

    fn root(&self) -> Location {
        Location::from_path(&self.0)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn named(name: &str) -> Query {
    Query {
        name: name.to_owned(),
        ..Query::default()
    }
}

fn search(root: &Location, query: &Query) -> (Vec<Hit>, Completion) {
    let hits = Arc::new(Mutex::new(Vec::new()));
    let (sender, finished) = mpsc::channel();
    let sender = Mutex::new(sender);
    let collector = Arc::clone(&hits);
    let running = start(
        root,
        query,
        Arc::new(move |hit| collector.lock().expect("hits lock").push(hit)),
        Arc::new(move |completion| {
            sender
                .lock()
                .expect("sender lock")
                .send(completion)
                .expect("test receiver alive");
        }),
    );
    let completion = finished
        .recv_timeout(Duration::from_secs(20))
        .expect("search finished in time");
    drop(running);
    let mut hits = std::mem::take(&mut *hits.lock().expect("hits lock"));
    hits.sort_by_key(|hit| hit.at.name());
    (hits, completion)
}

fn run(root: &Location, query: &Query) -> (Vec<String>, Completion) {
    let (hits, completion) = search(root, query);
    (hits.iter().map(|hit| hit.at.name()).collect(), completion)
}

#[test]
fn matches_name_substring() {
    let sandbox = Sandbox::new("names");
    sandbox.file("桜の写真.png", b"");
    sandbox.file("deep/写真集.txt", b"");
    sandbox.file("other.txt", b"");

    let (names, outcome) = run(&sandbox.root(), &named("写真"));

    assert_eq!(names, ["写真集.txt", "桜の写真.png"]);
    assert_eq!(outcome, Completion::Exhausted);
}

#[test]
fn content_search_returns_line() {
    let sandbox = Sandbox::new("contents");
    sandbox.file("one.txt", b"nothing here\n");
    sandbox.file("two.txt", b"first\nthe needle is here\nlast\n");

    let (hits, _) = search(
        &sandbox.root(),
        &Query {
            contents: "needle".to_owned(),
            ..Query::default()
        },
    );
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].at.name(), "two.txt");
    assert_eq!(hits[0].line.as_deref(), Some("the needle is here"));
    assert_eq!(hits[0].line_number, Some(2));
}

#[test]
fn honours_ignore_files() {
    let sandbox = Sandbox::new("ignored");
    sandbox.file(".gitignore", b"build/\n");
    sandbox.file("build/generated.txt", b"");
    sandbox.file("source.txt", b"");

    let (names, _) = run(&sandbox.root(), &named(".txt"));

    assert_eq!(names, ["source.txt"]);
}

#[test]
fn skips_hidden_unless_requested() {
    let sandbox = Sandbox::new("hidden");
    sandbox.file(".secret.txt", b"");
    sandbox.file("shown.txt", b"");

    let (names, _) = run(&sandbox.root(), &named(".txt"));
    assert_eq!(names, ["shown.txt"]);

    let mut all = named(".txt");
    all.options.hidden = true;
    let (names, _) = run(&sandbox.root(), &all);
    assert_eq!(names.len(), 2);
}

#[test]
fn stops_at_limit_and_reports() {
    let sandbox = Sandbox::new("limit");
    for index in 0..40 {
        sandbox.file(&format!("file{index}.txt"), b"");
    }
    let mut query = named(".txt");
    query.options.limit = 5;

    let (names, outcome) = run(&sandbox.root(), &query);

    assert_eq!(outcome, Completion::Truncated);
    assert!(names.len() <= 6, "{} hits", names.len());
}
