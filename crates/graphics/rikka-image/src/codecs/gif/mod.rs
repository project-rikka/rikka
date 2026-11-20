mod lzw;
mod quantize;
mod reader;
mod writer;

pub(crate) use reader::GifReader;
pub(crate) use writer::GifStream;

pub(crate) const EXTENSION: u8 = 0x21;
pub(crate) const IMAGE: u8 = 0x2C;
pub(crate) const TRAILER: u8 = 0x3B;
pub(crate) const GRAPHIC_CONTROL: u8 = 0xF9;
pub(crate) const APPLICATION: u8 = 0xFF;
pub(crate) const LOOPING: &[u8; 11] = b"NETSCAPE2.0";
pub(crate) const LOOP_COUNT: u8 = 1;
pub(crate) const COLOR_TABLE: u8 = 0x80;
pub(crate) const INTERLACED: u8 = 0x40;
pub(crate) const TABLE_SIZE: u8 = 0x07;
pub(crate) const TRANSPARENT: u8 = 0x01;
pub(crate) const DISPOSAL_SHIFT: u8 = 2;
pub(crate) const DISPOSAL_MASK: u8 = 0x07;
