use std::io::{ErrorKind, Write as _};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use rikka_worker::{StreamLimit, read_stream};

const LIMIT: StreamLimit = StreamLimit {
    time: Duration::from_secs(10),
    bytes: 1024 * 1024,
};

#[test]
fn endless_stream_is_refused_at_the_byte_limit() {
    let (reader, mut writer) = UnixStream::pair().expect("socket pair");
    let flooding = std::thread::spawn(move || {
        let chunk = [b'x'; 64 * 1024];
        while writer.write_all(&chunk).is_ok() {}
    });

    let read = read_stream(reader, LIMIT);

    assert_eq!(
        read.map_err(|error| error.kind()),
        Err(ErrorKind::FileTooLarge)
    );
    flooding
        .join()
        .expect("writer stops once the reader is gone");
}

#[test]
fn stream_within_the_limit_is_read_whole() {
    let (reader, mut writer) = UnixStream::pair().expect("socket pair");
    writer.write_all(b"clipping").expect("write");
    drop(writer);

    assert_eq!(read_stream(reader, LIMIT).expect("read"), b"clipping");
}
