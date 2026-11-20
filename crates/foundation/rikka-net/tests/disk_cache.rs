use std::time::Duration;

use rikka_net::DiskCache;

const KEPT_FOR: Duration = Duration::from_secs(60 * 60);

#[test]
fn entries_read_back_by_key() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let cache = DiskCache::open(folder.path().join("cache"), KEPT_FOR).expect("cache opens");

    cache
        .write("forecast-1.00_2.00", b"sunny")
        .expect("entry written");

    assert_eq!(
        cache.read("forecast-1.00_2.00").as_deref(),
        Some(&b"sunny"[..])
    );
    assert_eq!(cache.read("forecast-3.00_4.00"), None);
}
