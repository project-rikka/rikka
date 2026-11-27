use browser_store::{Error, PinnedItem, PinnedPlace, Store, TabRecord};

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
        clone.add_favorite(profile, format!("https://example.com/{page}"), "Page");
    }
    drop(clone);
    drop(store);

    let store = Store::open(&path).unwrap();
    assert_eq!(store.favorites(profile).unwrap().len(), 50);
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
        .execute_batch("DROP TABLE profile_settings; DROP INDEX favorites_profile;")
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
fn open_recreates_mismatched_tables_and_drops_undeclared_ones() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("browser.sqlite");
    let store = Store::open(&path).unwrap();
    let (_, space) = support::space(&store, "Home");
    store.flush().unwrap();
    drop(store);
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             DROP TABLE closed_tabs;
             CREATE TABLE closed_tabs (id INTEGER PRIMARY KEY, space_id INTEGER NOT NULL,
                 url TEXT NOT NULL, title TEXT NOT NULL, state BLOB, closed_at INTEGER NOT NULL);
             INSERT INTO closed_tabs VALUES (1, 1, 'https://old.example/', 'Old', x'00', 0);
             CREATE TABLE places (id INTEGER PRIMARY KEY, url TEXT NOT NULL);
             CREATE VIRTUAL TABLE places_text USING fts5(url, content = 'places');",
        )
        .unwrap();

    let store = Store::open(&path).unwrap();
    store.push_closed_tab(
        space,
        TabRecord {
            url: "https://new.example/".to_owned(),
            title: "New".to_owned(),
        },
    );
    store.flush().unwrap();
    let urls: Vec<String> = store
        .closed_tabs()
        .unwrap()
        .into_iter()
        .map(|closed| closed.url)
        .collect();
    assert_eq!(urls, ["https://new.example/"]);
    assert_eq!(store.profiles().unwrap()[0].name, "Default");
    drop(store);
    let leftovers: i64 = rusqlite::Connection::open(&path)
        .unwrap()
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name LIKE 'places%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(leftovers, 0);
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
