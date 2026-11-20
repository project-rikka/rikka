use std::io::{self, Read as _};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

const CHUNK: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamLimit {
    pub time: Duration,
    pub bytes: usize,
}

pub fn read_stream(mut stream: UnixStream, limit: StreamLimit) -> io::Result<Vec<u8>> {
    let deadline = Instant::now() + limit.time;
    let mut data = Vec::new();
    let mut chunk = vec![0; CHUNK];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        stream.set_read_timeout(Some(left))?;
        match stream.read(&mut chunk) {
            Ok(0) => return Ok(data),
            Ok(read) if data.len() + read > limit.bytes => {
                return Err(io::ErrorKind::FileTooLarge.into());
            }
            Ok(read) => data.extend_from_slice(&chunk[..read]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Err(io::ErrorKind::TimedOut.into());
            }
            Err(error) => return Err(error),
        }
    }
}
