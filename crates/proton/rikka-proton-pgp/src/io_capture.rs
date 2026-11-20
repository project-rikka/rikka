use std::io::{self, ErrorKind, Read, Write};

use crate::error::Error;

pub(crate) struct IoCapture<T> {
    inner: T,
    error: Option<io::Error>,
}

impl<T> IoCapture<T> {
    pub(crate) fn new(inner: T) -> Self {
        IoCapture { inner, error: None }
    }

    pub(crate) fn take_error(&mut self) -> Option<Error> {
        self.error.take().map(Error::Io)
    }

    fn record(&mut self, error: io::Error) -> io::Error {
        if error.kind() == ErrorKind::Interrupted {
            return error;
        }
        let forwarded = io::Error::new(error.kind(), error.to_string());
        self.error = Some(error);
        forwarded
    }
}

impl<R: Read> Read for IoCapture<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buffer).map_err(|error| self.record(error))
    }
}

impl<W: Write> Write for IoCapture<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.inner.write(buffer).map_err(|error| self.record(error))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush().map_err(|error| self.record(error))
    }
}
