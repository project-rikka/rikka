use browser_store::{Error, HistoryQuery, PinnedItem, PinnedPlace, Store, Transition, Visit};

use crate::support;

#[test]
fn open_creates_directories_and_persists() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("rikka/browser/browser.sqlite");
    let store = Store::open(&path).unwrap();
    let profile = store.create_profile("Work");
    store.flush().unwrap();
    drop(store);

    let store = Store::open(&path).unwrap();
    let profiles = store.profiles().unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].id, profile);
    assert_eq!(profiles[0].name, "Work");
}

#[test]
fn drop_commits_queued_writes() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("browser.sqlite");
    let store = Store::open(&path).unwrap();
    let profile = store.create_profile("Default");
    let clone = store.clone();
    for page in 0..50 {
        clone.record_visit(
            profile,
            Visit::new(format!("https://example.com/{page}"), Transition::Link),
        );
    }
    drop(clone);
    drop(store);

    let store = Store::open(&path).unwrap();
    assert_eq!(
        store
            .history(profile, &HistoryQuery::new(100))
            .unwrap()
            .len(),
        50
    );
}

#[test]
fn open_creates_missing_tables_and_indexes() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("browser.sqlite");
    let store = Store::open(&path).unwrap();
    let profile = store.create_profile("Work");
    store.flush().unwrap();
    drop(store);
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch("DROP TABLE profile_settings; DROP INDEX downloads_started;")
        .unwrap();

    let store = Store::open(&path).unwrap();
    store
        .set_profile_settings(profile, &vec!["ja".to_owned()])
        .unwrap();
    store.flush().unwrap();
    assert_eq!(
        store.profile_settings::<Vec<String>>(profile).unwrap(),
        Some(vec!["ja".to_owned()])
    );
    assert_eq!(store.profiles().unwrap()[0].name, "Work");
}

#[test]
fn open_recreates_mismatched_tables_and_their_dependents() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("browser.sqlite");
    let store = Store::open(&path).unwrap();
    let profile = store.create_profile("Work");
    store.record_visit(
        profile,
        Visit::new("https://example.com/old", Transition::Link),
    );
    store.flush().unwrap();
    drop(store);
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             DROP TABLE places;
             CREATE TABLE places (id INTEGER PRIMARY KEY, url TEXT NOT NULL);",
        )
        .unwrap();

    let store = Store::open(&path).unwrap();
    store.record_visit(
        profile,
        Visit::new("https://example.com/new", Transition::Link),
    );
    store.flush().unwrap();
    let urls: Vec<String> = store
        .history(profile, &HistoryQuery::new(10).text("example"))
        .unwrap()
        .into_iter()
        .map(|entry| entry.url)
        .collect();
    assert_eq!(urls, ["https://example.com/new"]);
    assert_eq!(store.profiles().unwrap()[0].name, "Work");
}

#[test]
fn flush_reports_failed_writes_once() {
    let store = Store::in_memory().unwrap();
    let (_, space) = support::space(&store, "Home");
    let outer = store.create_pinned_folder(PinnedPlace::root(space), "Outer");
    let inner = store.create_pinned_folder(PinnedPlace::folder(outer), "Inner");
    store.move_pinned(PinnedItem::Folder(outer), PinnedPlace::folder(inner));

    match store.flush() {
        Err(Error::WritesFailed { count, first }) => {
            assert_eq!(count, 1);
            assert!(matches!(*first, Error::FolderCycle(folder) if folder == outer));
        }
        other => panic!("expected WritesFailed, got {other:?}"),
    }
    store.flush().unwrap();
}

#[test]
fn failed_write_keeps_rest_of_batch() {
    let store = Store::in_memory().unwrap();
    let (_, space) = support::space(&store, "Home");
    let folder = store.create_pinned_folder(PinnedPlace::root(space), "Folder");
    store.move_pinned(PinnedItem::Folder(folder), PinnedPlace::folder(folder));
    let tab = store.pin_tab(PinnedPlace::root(space), "https://example.com/", None);

    assert!(store.flush().is_err());
    let tree = store.pinned(space).unwrap();
    assert_eq!(tree.len(), 2);
    assert!(matches!(&tree[1], browser_store::PinnedNode::Tab(pinned) if pinned.id == tab));
}

#[test]
fn coalesced_writes_keep_order() {
    let store = Store::in_memory().unwrap();
    let profile = store.create_profile("Default");
    store.record_visit(
        profile,
        Visit::new("https://example.com/", Transition::Link),
    );
    store.set_title(profile, "https://example.com/", "Old");
    store.delete_url(profile, "https://example.com/");
    store.record_visit(
        profile,
        Visit::new("https://example.com/", Transition::Link),
    );
    store.set_title(profile, "https://example.com/", "New");
    store.flush().unwrap();

    let history = store.history(profile, &HistoryQuery::new(10)).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].title, "New");
}
