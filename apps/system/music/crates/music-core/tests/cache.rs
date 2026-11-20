use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use music_core::AudioCache;

const MIB: u64 = 1024 * 1024;

fn staged(folder: &Path, name: &str, size: u64) -> PathBuf {
    let path = folder.join(name);
    fs::write(&path, vec![0; size as usize]).unwrap();
    path
}

fn aged(path: &Path, seconds: u64) {
    let then = SystemTime::now() - Duration::from_secs(seconds);
    let file = File::options().write(true).open(path).unwrap();
    file.set_times(fs::FileTimes::new().set_accessed(then).set_modified(then))
        .unwrap();
}

#[test]
fn kept_file_found_by_key() {
    let root = tempfile::tempdir().unwrap();
    let cache = AudioCache::open(root.path().join("audio"), 10 * MIB);
    let shelf = cache.for_provider("qobuz");
    let spool = shelf.spool().unwrap();

    shelf
        .insert("123.highest", &staged(&spool, "1.audio", 10))
        .unwrap();

    let path = shelf.find("123.highest").unwrap();
    assert_eq!(fs::read(path).unwrap().len(), 10);
    assert!(shelf.find("123.low").is_none());
    assert!(!spool.join("1.audio").exists());
}

#[test]
fn key_stays_inside_provider() {
    let root = tempfile::tempdir().unwrap();
    let cache = AudioCache::open(root.path().join("audio"), 10 * MIB);
    let shelf = cache.for_provider("../qobuz");
    let spool = shelf.spool().unwrap();

    shelf
        .insert("../../escaped", &staged(&spool, "1.audio", 10))
        .unwrap();

    let path = shelf.find("../../escaped").unwrap();
    assert!(path.starts_with(root.path().join("audio")));
    assert_eq!(path.parent(), Some(shelf.folder()));
    assert!(!root.path().join("escaped").exists());
}

#[test]
fn prunes_least_recently_used_first() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("spotify");
    fs::create_dir_all(&folder).unwrap();
    let old = staged(&folder, "old", MIB);
    let recent = staged(&folder, "recent", MIB);
    aged(&old, 600);
    aged(&recent, 60);

    let cache = AudioCache::open(root.path(), MIB);
    assert!(cache.prune_and_measure() <= MIB);

    assert!(!old.exists());
    assert!(recent.exists());
}

#[test]
fn clear_spares_active_spools() {
    let root = tempfile::tempdir().unwrap();
    let cache = AudioCache::open(root.path(), 10 * MIB);
    let shelf = cache.for_provider("qobuz");
    let spool = shelf.spool().unwrap();
    shelf
        .insert("1.standard", &staged(&spool, "a", 100))
        .unwrap();
    let playing = staged(&spool, "b", 50);
    assert_eq!(cache.prune_and_measure(), 100);

    cache.clear().unwrap();

    assert_eq!(cache.prune_and_measure(), 0);
    assert!(shelf.find("1.standard").is_none());
    assert!(playing.exists());
}
