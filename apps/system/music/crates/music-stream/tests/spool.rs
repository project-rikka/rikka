use std::io::Read;

use music_stream::Spool;

#[test]
fn abandoned_download_fails_reader() {
    let folder = tempfile::tempdir().unwrap();
    let mut spool = Spool::create(&folder.path().join("1.audio")).unwrap();
    let mut stored = spool.stored().unwrap();
    spool.begin(Some(10));
    spool.write(b"ab").unwrap();
    drop(spool);

    let mut read = Vec::new();
    let broken = stored.read_to_end(&mut read);

    assert_eq!(read, b"ab");
    assert!(broken.is_err());
}
