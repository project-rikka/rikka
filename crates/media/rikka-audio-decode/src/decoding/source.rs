use rikka_sync::{Mutex, MutexGuard};
use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::sync::Arc;

pub(crate) trait Media: Read + Seek + Send + Sync {
    fn byte_len(&self) -> Option<u64>;
}

impl Media for File {
    fn byte_len(&self) -> Option<u64> {
        self.metadata().ok().map(|metadata| metadata.len())
    }
}

impl Media for Cursor<Arc<[u8]>> {
    fn byte_len(&self) -> Option<u64> {
        Some(self.get_ref().len() as u64)
    }
}

#[derive(Clone)]
pub(crate) struct Shared(Arc<Mutex<Box<dyn Media>>>);

impl Shared {
    pub(crate) fn new(media: Box<dyn Media>) -> Shared {
        Shared(Arc::new(Mutex::new(media)))
    }

    pub(crate) fn rewind(&self) -> std::io::Result<()> {
        self.lock().seek(SeekFrom::Start(0)).map(drop)
    }

    pub(crate) fn len(&self) -> Option<u64> {
        self.lock().byte_len()
    }

    fn lock(&self) -> MutexGuard<'_, Box<dyn Media>> {
        self.0.lock()
    }
}

impl Read for Shared {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.lock().read(buf)
    }
}

impl Seek for Shared {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.lock().seek(position)
    }
}

impl rikka_ffmpeg::ByteSource for Shared {
    fn byte_len(&self) -> Option<u64> {
        self.len()
    }
}
