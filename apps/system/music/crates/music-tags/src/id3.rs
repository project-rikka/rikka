use std::io::Cursor;

use lofty::file::{AudioFile, TaggedFile};
use lofty::mpeg::MpegFile;
use lofty::tag::Tag;

use crate::container;

const MAGIC: &[u8] = b"ID3";
const TRAILER: usize = 256;

pub(crate) fn tags(mut bytes: Vec<u8>) -> Vec<Tag> {
    if !bytes.starts_with(MAGIC) {
        return Vec::new();
    }
    bytes.resize(bytes.len() + TRAILER, 0);
    let options = container::options().read_properties(false);
    let Ok(file) = MpegFile::read_from(&mut Cursor::new(bytes), options) else {
        return Vec::new();
    };
    let mut tagged: TaggedFile = file.into();
    container::taken(&mut tagged)
}
