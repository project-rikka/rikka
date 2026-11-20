use std::path::Path;

use rikka_watch::{Change, ChangeKind, Error, Event, Reach, Watches};

fn changes(watches: &mut Watches) -> Vec<(std::path::PathBuf, ChangeKind)> {
    watches
        .read()
        .unwrap()
        .into_iter()
        .map(|event| match event {
            Event::Changed(Change { path, kind }) => (path, kind),
            Event::Lost => panic!("events were lost"),
        })
        .collect()
}

fn saw(seen: &[(std::path::PathBuf, ChangeKind)], path: &Path, kind: ChangeKind) -> bool {
    seen.iter().any(|(at, what)| at == path && *what == kind)
}

#[test]
fn folder_reports_entries_and_contents_by_path() {
    let folder = tempfile::tempdir().unwrap();
    let mut watches = Watches::new().unwrap();
    watches.watch(folder.path(), Reach::Contents).unwrap();

    let note = folder.path().join("note.txt");
    std::fs::write(&note, "one").unwrap();
    let moved = folder.path().join("moved.txt");
    std::fs::rename(&note, &moved).unwrap();
    std::fs::remove_file(&moved).unwrap();

    let seen = changes(&mut watches);
    assert!(saw(&seen, &note, ChangeKind::Created), "{seen:?}");
    assert!(saw(&seen, &note, ChangeKind::Written), "{seen:?}");
    assert!(saw(&seen, &note, ChangeKind::Removed), "{seen:?}");
    assert!(saw(&seen, &moved, ChangeKind::Created), "{seen:?}");
    assert!(saw(&seen, &moved, ChangeKind::Removed), "{seen:?}");
    assert!(changes(&mut watches).is_empty());
}

#[test]
fn entries_reach_ignores_writes() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("kept.txt");
    std::fs::write(&file, "one").unwrap();
    let mut watches = Watches::new().unwrap();
    watches.watch(folder.path(), Reach::Entries).unwrap();

    std::fs::write(&file, "two").unwrap();

    assert!(changes(&mut watches).is_empty());
}

#[test]
fn tree_follows_folders_made_and_moved_after_watching() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let mut watches = Watches::new().unwrap();
    watches.watch(root.path(), Reach::Tree).unwrap();

    let album = root.path().join("album");
    std::fs::create_dir(&album).unwrap();
    std::fs::write(album.join("early.flac"), "x").unwrap();
    let seen = changes(&mut watches);
    assert!(saw(&seen, &album, ChangeKind::Created), "{seen:?}");
    assert!(
        saw(&seen, &album.join("early.flac"), ChangeKind::Created),
        "a file made before the new folder was watched is still reported: {seen:?}"
    );

    std::fs::write(album.join("late.flac"), "x").unwrap();
    let seen = changes(&mut watches);
    assert!(
        saw(&seen, &album.join("late.flac"), ChangeKind::Written),
        "{seen:?}"
    );

    let away = outside.path().join("album");
    std::fs::rename(&album, &away).unwrap();
    assert!(saw(&changes(&mut watches), &album, ChangeKind::Removed));
    std::fs::write(away.join("elsewhere.flac"), "x").unwrap();
    assert!(
        changes(&mut watches).is_empty(),
        "a folder moved out is no longer watched"
    );
}

#[test]
fn unwatch_stops_reports_and_missing_path_is_not_found() {
    let folder = tempfile::tempdir().unwrap();
    let mut watches = Watches::new().unwrap();
    watches.watch(folder.path(), Reach::Contents).unwrap();
    watches.unwatch(folder.path());
    std::fs::write(folder.path().join("quiet.txt"), "x").unwrap();
    assert!(changes(&mut watches).is_empty());

    assert!(matches!(
        watches.watch(&folder.path().join("missing"), Reach::Entries),
        Err(Error::NotFound(_))
    ));
}
