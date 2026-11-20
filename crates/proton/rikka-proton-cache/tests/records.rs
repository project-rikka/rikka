use std::os::unix::fs::PermissionsExt as _;

use rikka_proton_cache::{Cache, Table};
use rikka_proton_pgp::SymmetricKey;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Note {
    title: String,
    pages: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Reshaped {
    heading: String,
}

const NOTES: Table<Note> = Table::new("notes");
const NOTES_RESHAPED: Table<Reshaped> = Table::new("notes");
const RECENT: Table<Note> = Table::bounded("recent", 2);

fn note(title: &str, pages: u32) -> Note {
    Note {
        title: title.to_owned(),
        pages,
    }
}

#[test]
fn records_round_trip_in_id_order() {
    let cache = Cache::in_memory(SymmetricKey::generate()).unwrap();
    cache.put(&NOTES, "b", note("second", 2));
    cache.put(&NOTES, "a", note("first", 1));
    cache.put(&NOTES, "c", note("third", 3));
    cache.remove(&NOTES, "c");

    assert_eq!(cache.get(&NOTES, "a"), Some(note("first", 1)));
    assert_eq!(cache.get(&NOTES, "c"), None);
    cache.flush().unwrap();

    let listed = cache.entries(&NOTES);
    assert_eq!(
        listed,
        vec![
            ("a".to_owned(), note("first", 1)),
            ("b".to_owned(), note("second", 2))
        ]
    );

    cache.replace(&NOTES, [("z".to_owned(), note("only", 9))]);
    assert_eq!(cache.values(&NOTES), vec![note("only", 9)]);
    cache.flush().unwrap();
    assert_eq!(cache.values(&NOTES), vec![note("only", 9)]);
}

#[test]
fn records_persist_sealed_and_private() {
    let dir = tempfile::tempdir().unwrap();
    let key = SymmetricKey::generate();
    {
        let cache = Cache::open(dir.path().join("account"), key.clone()).unwrap();
        cache.put(&NOTES, "kept", note("a secret title", 7));
        cache.flush().unwrap();
    }

    let file = dir.path().join("account").join("cache.sqlite");
    let mode = std::fs::metadata(&file).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    let folder = std::fs::metadata(dir.path().join("account"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(folder & 0o777, 0o700);

    let raw = std::fs::read(&file).unwrap();
    let wal = std::fs::read(file.with_extension("sqlite-wal")).unwrap_or_default();
    for bytes in [&raw, &wal] {
        assert!(
            !bytes.windows(14).any(|window| window == b"a secret title"),
            "plaintext found on disk"
        );
        assert!(!bytes.windows(4).any(|window| window == b"kept"));
    }

    let reopened = Cache::open(dir.path().join("account"), key).unwrap();
    assert_eq!(
        reopened.get(&NOTES, "kept"),
        Some(note("a secret title", 7))
    );

    let stranger = Cache::open(dir.path().join("account"), SymmetricKey::generate()).unwrap();
    assert_eq!(stranger.get(&NOTES, "kept"), None);
    assert!(stranger.entries(&NOTES).is_empty());
}

#[test]
fn reshaped_records_read_as_missing_and_are_discarded() {
    let dir = tempfile::tempdir().unwrap();
    let key = SymmetricKey::generate();
    {
        let cache = Cache::open(dir.path(), key.clone()).unwrap();
        cache.put(&NOTES, "old", note("old shape", 1));
        cache.flush().unwrap();
    }

    let reshaped = Cache::open(dir.path(), key.clone()).unwrap();
    assert_eq!(reshaped.get(&NOTES_RESHAPED, "old"), None);
    reshaped.flush().unwrap();
    assert_eq!(reshaped.get(&NOTES, "old"), None);
    let fresh = Reshaped {
        heading: "new shape".into(),
    };
    reshaped.put(&NOTES_RESHAPED, "old", fresh.clone());
    reshaped.flush().unwrap();
    drop(reshaped);

    let again = Cache::open(dir.path(), key).unwrap();
    assert_eq!(again.values(&NOTES_RESHAPED), vec![fresh]);
}

#[test]
fn batch_is_atomic() {
    let cache = Cache::in_memory(SymmetricKey::generate()).unwrap();
    cache.put(&NOTES, "gone", note("gone", 0));
    let mut batch = cache.batch();
    batch
        .put(&NOTES, "one", note("one", 1))
        .put(&NOTES, "two", note("two", 2))
        .remove(&NOTES, "gone");
    assert_eq!(cache.get(&NOTES, "one"), None);
    batch.commit();
    cache.flush().unwrap();

    assert_eq!(cache.values(&NOTES), vec![note("one", 1), note("two", 2)]);
}

#[test]
fn failed_batch_lands_once_the_store_recovers() {
    let dir = tempfile::tempdir().unwrap();
    let key = SymmetricKey::generate();
    let cache = Cache::open(dir.path(), key.clone()).unwrap();
    cache.put(&NOTES, "a", note("first", 1));
    cache.flush().unwrap();

    let outside = rusqlite::Connection::open(cache.path().unwrap()).unwrap();
    outside
        .execute_batch("ALTER TABLE records RENAME TO parked")
        .unwrap();
    cache.replace(&NOTES, [("b".to_owned(), note("second", 2))]);
    assert!(cache.flush().is_err());
    assert_eq!(cache.values(&NOTES), [note("second", 2)]);

    outside
        .execute_batch("ALTER TABLE parked RENAME TO records")
        .unwrap();
    cache.flush().unwrap();

    let reopened = Cache::open(dir.path(), key).unwrap();
    assert_eq!(reopened.values(&NOTES), [note("second", 2)]);
}

#[test]
fn bounded_tables_keep_the_most_recently_used_records() {
    let dir = tempfile::tempdir().unwrap();
    let key = SymmetricKey::generate();
    {
        let cache = Cache::open(dir.path(), key.clone()).unwrap();
        cache.put(&RECENT, "a", note("first", 1));
        cache.put(&RECENT, "b", note("second", 2));
        cache.flush().unwrap();
        assert!(cache.get(&RECENT, "a").is_some());
        cache.put(&RECENT, "c", note("third", 3));
        cache.put(&NOTES, "unbounded", note("kept", 4));
        cache.flush().unwrap();
    }

    let reopened = Cache::open(dir.path(), key).unwrap();
    assert_eq!(reopened.get(&RECENT, "a"), Some(note("first", 1)));
    assert_eq!(reopened.get(&RECENT, "b"), None);
    assert_eq!(reopened.get(&RECENT, "c"), Some(note("third", 3)));
    assert_eq!(reopened.get(&NOTES, "unbounded"), Some(note("kept", 4)));
}

#[test]
fn cache_of_another_shape_is_rebuilt() {
    let dir = tempfile::tempdir().unwrap();
    let key = SymmetricKey::generate();
    {
        let older = rusqlite::Connection::open(dir.path().join("cache.sqlite")).unwrap();
        older
            .execute_batch(
                "CREATE TABLE records (
                    tbl TEXT NOT NULL,
                    key BLOB NOT NULL,
                    sealed BLOB NOT NULL,
                    PRIMARY KEY (tbl, key)
                ) WITHOUT ROWID;
                INSERT INTO records VALUES ('notes', x'00', x'00');
                PRAGMA user_version = 1;",
            )
            .unwrap();
    }

    let cache = Cache::open(dir.path(), key.clone()).unwrap();
    cache.put(&NOTES, "fresh", note("rebuilt", 1));
    cache.flush().unwrap();
    assert_eq!(cache.entries(&NOTES).len(), 1);
    drop(cache);

    let reopened = Cache::open(dir.path(), key).unwrap();
    assert_eq!(reopened.get(&NOTES, "fresh"), Some(note("rebuilt", 1)));
}
