use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rikka_xdg::Watch;

fn wait(changes: &AtomicUsize, count: usize) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if changes.load(Ordering::Relaxed) >= count {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn replaced_symlink_is_change() {
    let root = tempfile::tempdir().expect("a directory to work in");
    let installed = root.path().join("one/applications");
    let then_installed = root.path().join("two/applications");
    std::fs::create_dir_all(&installed).expect("the first generation");
    std::fs::create_dir_all(&then_installed).expect("the second");
    let profile = root.path().join("profile");
    std::os::unix::fs::symlink(root.path().join("one"), &profile).expect("the link");

    let changes = Arc::new(AtomicUsize::new(0));
    let counting = Arc::clone(&changes);
    let _watch = Watch::folder(&profile.join("applications"), move || {
        counting.fetch_add(1, Ordering::Relaxed);
    })
    .expect("a watch");

    std::fs::remove_file(&profile).expect("a generation is a link replaced");
    std::os::unix::fs::symlink(root.path().join("two"), &profile).expect("the next one");

    assert!(wait(&changes, 1), "replacing the link went unheard");
}

#[test]
fn new_entry_is_change() {
    let root = tempfile::tempdir().expect("a directory to work in");
    let applications = root.path().join("applications");
    std::fs::create_dir(&applications).expect("somewhere to install into");

    let changes = Arc::new(AtomicUsize::new(0));
    let counting = Arc::clone(&changes);
    let _watch = Watch::folder(&applications, move || {
        counting.fetch_add(1, Ordering::Relaxed);
    })
    .expect("a watch");

    std::fs::write(applications.join("thing.desktop"), "[Desktop Entry]\n")
        .expect("something installed");

    assert!(wait(&changes, 1), "the entry went unheard");
}

#[test]
fn reading_directory_is_not_change() {
    let root = tempfile::tempdir().expect("a directory to work in");
    let applications = root.path().join("applications");
    std::fs::create_dir_all(applications.join("nested")).expect("somewhere installed");
    let entry = applications.join("thing.desktop");
    std::fs::write(&entry, "[Desktop Entry]\n").expect("something installed");

    let changes = Arc::new(AtomicUsize::new(0));
    let counting = Arc::clone(&changes);
    let _watch = Watch::folder(&applications, move || {
        counting.fetch_add(1, Ordering::Relaxed);
    })
    .expect("a watch");

    for _ in 0..5 {
        std::fs::read(&entry).expect("the entry reads");
        std::fs::read_dir(&applications)
            .expect("the folder lists")
            .for_each(drop);
    }

    std::thread::sleep(Duration::from_millis(800));
    assert_eq!(
        changes.load(Ordering::Relaxed),
        0,
        "reading was taken as a change"
    );
}

#[test]
fn rewritten_file_is_change_neighbours_not() {
    let root = tempfile::tempdir().expect("a directory to work in");
    let list = root.path().join("mimeapps.list");
    let neighbour = root.path().join("other.conf");
    std::fs::write(&list, "[Default Applications]\n").expect("the list");
    std::fs::write(&neighbour, "").expect("the neighbour");

    let changes = Arc::new(AtomicUsize::new(0));
    let counting = Arc::clone(&changes);
    let _watch = Watch::folder(&list, move || {
        counting.fetch_add(1, Ordering::Relaxed);
    })
    .expect("a watch");

    std::fs::write(&neighbour, "changed").expect("the neighbour changes");
    std::thread::sleep(Duration::from_millis(800));
    assert_eq!(
        changes.load(Ordering::Relaxed),
        0,
        "a neighbour was taken as a change"
    );

    std::fs::write(&list, "[Default Applications]\ntext/html=a.desktop;\n")
        .expect("the list is rewritten in place");
    assert!(wait(&changes, 1), "rewriting the list went unheard");
}

#[test]
fn missing_files_returning_are_change() {
    let root = tempfile::tempdir().expect("a directory to work in");
    let moved = root.path().join("drive/shoot/clip.mp4");
    let beside = root.path().join("beside.mp4");

    let changes = Arc::new(AtomicUsize::new(0));
    let counting = Arc::clone(&changes);
    let _watch = Watch::paths(vec![moved.clone(), beside.clone()], move || {
        counting.fetch_add(1, Ordering::Relaxed);
    })
    .expect("a watch");

    std::fs::write(&beside, "").expect("one file returns");
    assert!(wait(&changes, 1), "the returning file went unheard");

    std::fs::create_dir_all(moved.parent().expect("a folder")).expect("its folders");
    std::fs::write(&moved, "").expect("the other returns");
    assert!(wait(&changes, 2), "the file in new folders went unheard");
}
