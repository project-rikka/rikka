use rikka_proton_cache::{BlobArea, Cache};
use rikka_proton_pgp::SymmetricKey;

const SEALING_OVERHEAD: u64 = 28;
const THUMBNAILS: BlobArea = BlobArea::new("thumbnails", 3 * (100 + SEALING_OVERHEAD));

#[test]
fn evicts_least_recently_used_blobs() {
    let dir = tempfile::tempdir().unwrap();
    let key = SymmetricKey::generate();
    let cache = Cache::open(dir.path(), key.clone()).unwrap();

    cache.put_blob(&THUMBNAILS, "a", vec![1; 100]);
    cache.put_blob(&THUMBNAILS, "b", vec![2; 100]);
    cache.put_blob(&THUMBNAILS, "c", vec![3; 100]);
    cache.flush().unwrap();

    assert_eq!(cache.blob(&THUMBNAILS, "a"), Some(vec![1; 100]));
    cache.put_blob(&THUMBNAILS, "d", vec![4; 100]);
    cache.flush().unwrap();

    assert_eq!(cache.blob(&THUMBNAILS, "b"), None);
    assert_eq!(cache.blob(&THUMBNAILS, "a"), Some(vec![1; 100]));
    assert_eq!(cache.blob(&THUMBNAILS, "c"), Some(vec![3; 100]));
    assert_eq!(cache.blob(&THUMBNAILS, "d"), Some(vec![4; 100]));

    cache.put_blob(&THUMBNAILS, "huge", vec![0; 1000]);
    cache.remove_blob(&THUMBNAILS, "c");
    cache.flush().unwrap();
    drop(cache);

    let reopened = Cache::open(dir.path(), key).unwrap();
    assert_eq!(reopened.blob(&THUMBNAILS, "huge"), None);
    assert_eq!(reopened.blob(&THUMBNAILS, "c"), None);
    assert_eq!(reopened.blob(&THUMBNAILS, "d"), Some(vec![4; 100]));
}
