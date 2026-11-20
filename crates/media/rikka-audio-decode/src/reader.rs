use std::io::{Read, Seek, SeekFrom};

use crate::decoder::Decoder;
use crate::decoding::{Input, Media, Pipeline};
use crate::error::Result;

pub trait StreamReader: Read + Seek + Send + Sync {
    fn complete_len(&self) -> Option<u64>;
}

impl<R: StreamReader + ?Sized> StreamReader for Box<R> {
    fn complete_len(&self) -> Option<u64> {
        (**self).complete_len()
    }
}

pub fn reader_decoder(reader: impl StreamReader + 'static) -> Result<Decoder> {
    Ok(Decoder::new(Pipeline::open(Input::reader(reader))?))
}

pub(crate) fn media(reader: impl StreamReader + 'static) -> Box<dyn Media> {
    Box::new(Reader(reader))
}

struct Reader<R>(R);

impl<R: StreamReader> Read for Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl<R: StreamReader> Seek for Reader<R> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.0.seek(position)
    }
}

impl<R: StreamReader> Media for Reader<R> {
    fn byte_len(&self) -> Option<u64> {
        self.0.complete_len()
    }
}
