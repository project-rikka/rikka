use crate::checksum::{adler32, crc32};
use crate::deflate::{Deflater, Level};
use crate::error::{Error, Result};
use crate::inflate::Inflater;

const ZLIB_DEFLATE: u8 = 8;
const ZLIB_MAX_WINDOW: u8 = 7;
const ZLIB_CHECK_DIVISOR: u16 = 31;
const ZLIB_PRESET_DICTIONARY: u8 = 0x20;
const ZLIB_METHOD: u8 = ZLIB_MAX_WINDOW << 4 | ZLIB_DEFLATE;
const ZLIB_TRAILER: usize = 4;
const GZIP_MAGIC: [u8; 3] = [0x1F, 0x8B, 0x08];
const GZIP_HEADER: usize = 10;
const GZIP_TRAILER: usize = 8;
const GZIP_UNKNOWN_SYSTEM: u8 = 255;
const FLAG_HEADER_CRC: u8 = 0x02;
const FLAG_EXTRA: u8 = 0x04;
const FLAG_NAME: u8 = 0x08;
const FLAG_COMMENT: u8 = 0x10;
const FLAG_RESERVED: u8 = 0xE0;

impl Inflater {
    pub fn zlib(&mut self, data: &[u8], out: &mut Vec<u8>, limit: usize) -> Result<()> {
        let [method, flags, ..] = *data else {
            return Err(Error::Truncated);
        };
        let valid = method & 0x0F == ZLIB_DEFLATE
            && method >> 4 <= ZLIB_MAX_WINDOW
            && (u16::from(method) << 8 | u16::from(flags)) % ZLIB_CHECK_DIVISOR == 0
            && flags & ZLIB_PRESET_DICTIONARY == 0;
        if !valid {
            return Err(Error::UnsupportedHeader);
        }
        let body = &data[2..];
        let start = out.len();
        let consumed = self.inflate(body, out, limit)?;
        let trailer = body
            .get(consumed..)
            .and_then(|rest| rest.first_chunk::<ZLIB_TRAILER>())
            .ok_or(Error::Truncated)?;
        if u32::from_be_bytes(*trailer) != adler32(&out[start..]) {
            return Err(Error::Checksum);
        }
        Ok(())
    }

    pub fn gzip(&mut self, data: &[u8], out: &mut Vec<u8>, limit: usize) -> Result<()> {
        if data.len() < GZIP_HEADER + GZIP_TRAILER {
            return Err(Error::Truncated);
        }
        let flags = data[3];
        if data[..3] != GZIP_MAGIC || flags & FLAG_RESERVED != 0 {
            return Err(Error::UnsupportedHeader);
        }
        let mut at = GZIP_HEADER;
        if flags & FLAG_EXTRA != 0 {
            let extra = data
                .get(at..)
                .and_then(|rest| rest.first_chunk::<2>())
                .ok_or(Error::Truncated)?;
            at += 2 + usize::from(u16::from_le_bytes(*extra));
        }
        for flag in [FLAG_NAME, FLAG_COMMENT] {
            if flags & flag != 0 {
                let rest = data.get(at..).ok_or(Error::Truncated)?;
                let end = rest
                    .iter()
                    .position(|&byte| byte == 0)
                    .ok_or(Error::Truncated)?;
                at += end + 1;
            }
        }
        if flags & FLAG_HEADER_CRC != 0 {
            at += 2;
        }
        let body = data.get(at..).ok_or(Error::Truncated)?;
        let start = out.len();
        let consumed = self.inflate(body, out, limit)?;
        let trailer = body
            .get(consumed..)
            .and_then(|rest| rest.first_chunk::<GZIP_TRAILER>())
            .ok_or(Error::Truncated)?;
        let checksum = u32::from_le_bytes([trailer[0], trailer[1], trailer[2], trailer[3]]);
        let size = u32::from_le_bytes([trailer[4], trailer[5], trailer[6], trailer[7]]);
        let produced = &out[start..];
        if checksum != crc32(produced) || size != produced.len() as u32 {
            return Err(Error::Checksum);
        }
        Ok(())
    }
}

impl Deflater {
    pub fn zlib(&mut self, data: &[u8], out: &mut Vec<u8>) {
        let level: u8 = match self.level() {
            Level::Fast => 0,
            Level::Balanced => 2,
            Level::Best => 3,
        };
        let flags = level << 6;
        let header = u16::from(ZLIB_METHOD) << 8 | u16::from(flags);
        let check = (ZLIB_CHECK_DIVISOR - header % ZLIB_CHECK_DIVISOR) % ZLIB_CHECK_DIVISOR;
        out.extend_from_slice(&[ZLIB_METHOD, flags | check as u8]);
        self.deflate(data, out);
        out.extend_from_slice(&adler32(data).to_be_bytes());
    }

    pub fn gzip(&mut self, data: &[u8], out: &mut Vec<u8>) {
        let effort: u8 = match self.level() {
            Level::Fast => 4,
            Level::Balanced => 0,
            Level::Best => 2,
        };
        out.extend_from_slice(&GZIP_MAGIC);
        out.extend_from_slice(&[0, 0, 0, 0, 0, effort, GZIP_UNKNOWN_SYSTEM]);
        self.deflate(data, out);
        out.extend_from_slice(&crc32(data).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    }
}
