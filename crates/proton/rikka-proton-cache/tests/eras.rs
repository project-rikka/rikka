use rikka_proton_cache::{BlobArea, Cache, Table};
use rikka_proton_pgp::SymmetricKey;

const NAMES: Table<String> = Table::new("names");
const PICTURES: BlobArea = BlobArea::new("pictures", 1024);

#[test]
fn retire_clears_and_drops_late_writes() {
    let dir = tempfile::tempdir().unwrap();
    let key = SymmetricKey::generate();
    let cache = Cache::open(dir.path(), key.clone()).unwrap();
    let older = cache.current_era();
    older.put(&NAMES, "who", "Alice".to_owned());
    older.put_blob(&PICTURES, "face", vec![1, 2, 3]);
    older.flush().unwrap();

    cache.retire().unwrap();
    assert!(!older.is_current());
    older.put(&NAMES, "late", "Mallory".to_owned());
    older.put_blob(&PICTURES, "late", vec![9]);
    older.flush().unwrap();

    let newer = cache.current_era();
    assert!(newer.is_current());
    assert_eq!(newer.get(&NAMES, "who"), None);
    assert_eq!(newer.get(&NAMES, "late"), None);
    assert_eq!(newer.blob(&PICTURES, "face"), None);
    assert_eq!(newer.blob(&PICTURES, "late"), None);

    newer.put(&NAMES, "who", "Bob".to_owned());
    newer.flush().unwrap();
    drop((cache, older, newer));

    let reopened = Cache::open(dir.path(), key).unwrap();
    assert_eq!(
        reopened.entries(&NAMES),
        vec![("who".to_owned(), "Bob".to_owned())]
    );
}

#[test]
fn clear_keeps_era() {
    let cache = Cache::in_memory(SymmetricKey::generate()).unwrap();
    let handle = cache.current_era();
    handle.put(&NAMES, "who", "Alice".to_owned());
    cache.clear().unwrap();

    assert!(handle.is_current());
    assert_eq!(handle.get(&NAMES, "who"), None);
    handle.put(&NAMES, "who", "Carol".to_owned());
    assert_eq!(handle.get(&NAMES, "who"), Some("Carol".to_owned()));
}
