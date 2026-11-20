use crate::error::{Error, Result};
use crate::io::mac_roman;
use crate::key::Key;

#[derive(Clone)]
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    base: u64,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            base: 0,
        }
    }

    pub(crate) fn offset(&self) -> u64 {
        self.base + self.pos as u64
    }

    pub(crate) fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub(crate) fn rest(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }

    pub(crate) fn malformed(&self, what: &'static str) -> Error {
        Error::Malformed {
            what,
            offset: self.offset(),
        }
    }

    pub(crate) fn bytes(&mut self, count: usize) -> Result<&'a [u8]> {
        if count > self.remaining() {
            return Err(Error::Truncated {
                offset: self.offset(),
                needed: count as u64,
            });
        }
        let slice = &self.data[self.pos..self.pos + count];
        self.pos += count;
        Ok(slice)
    }

    pub(crate) fn take_rest(&mut self) -> &'a [u8] {
        let rest = self.rest();
        self.pos = self.data.len();
        rest
    }

    pub(crate) fn skip(&mut self, count: usize) -> Result<()> {
        self.bytes(count).map(|_| ())
    }

    pub(crate) fn sub(&mut self, length: u64) -> Result<Reader<'a>> {
        let length = usize::try_from(length).map_err(|_| Error::Oversized)?;
        let base = self.offset();
        let data = self.bytes(length)?;
        Ok(Reader { data, pos: 0, base })
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let slice = self.bytes(N)?;
        let mut array = [0; N];
        array.copy_from_slice(slice);
        Ok(array)
    }

    pub(crate) fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    pub(crate) fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }

    pub(crate) fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    pub(crate) fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_be_bytes(self.array()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    pub(crate) fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.array()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    pub(crate) fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.array()?))
    }

    pub(crate) fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_be_bytes(self.array()?))
    }

    pub(crate) fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_be_bytes(self.array()?))
    }

    pub(crate) fn length(&mut self, wide: bool) -> Result<u64> {
        if wide {
            self.u64()
        } else {
            self.u32().map(u64::from)
        }
    }

    pub(crate) fn key(&mut self) -> Result<Key> {
        Ok(Key(self.array()?))
    }

    pub(crate) fn peek_key(&self) -> Option<Key> {
        let slice = self.data.get(self.pos..self.pos + 4)?;
        Some(Key([slice[0], slice[1], slice[2], slice[3]]))
    }

    pub(crate) fn expect(&mut self, expected: &[u8; 4]) -> Result<()> {
        let key = self.key()?;
        if key.0 == *expected {
            Ok(())
        } else {
            Err(Error::Signature {
                expected: Key(*expected),
                found: key,
            })
        }
    }

    pub(crate) fn pascal_bytes(&mut self, alignment: usize) -> Result<&'a [u8]> {
        let length = usize::from(self.u8()?);
        let text = self.bytes(length)?;
        let consumed = length + 1;
        let padded = consumed.div_ceil(alignment.max(1)) * alignment.max(1);
        let padding = (padded - consumed).min(self.remaining());
        self.skip(padding)?;
        Ok(text)
    }

    pub(crate) fn pascal(&mut self, alignment: usize) -> Result<String> {
        Ok(mac_roman::decode(self.pascal_bytes(alignment)?))
    }

    pub(crate) fn utf16(&mut self, units: usize) -> Result<String> {
        let bytes = self.bytes(units.checked_mul(2).ok_or(Error::Oversized)?)?;
        let mut code_units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_be_bytes(*pair))
            .collect();
        while code_units.last() == Some(&0) {
            code_units.pop();
        }
        Ok(String::from_utf16_lossy(&code_units))
    }

    pub(crate) fn unicode(&mut self) -> Result<String> {
        let units = self.u32()? as usize;
        if units * 2 > self.remaining() {
            return Err(self.malformed("unicode string"));
        }
        self.utf16(units)
    }

    pub(crate) fn rect(&mut self) -> Result<crate::geometry::Rect> {
        let top = self.i32()?;
        let left = self.i32()?;
        let bottom = self.i32()?;
        let right = self.i32()?;
        Ok(crate::geometry::Rect {
            top,
            left,
            bottom,
            right,
        })
    }

    pub(crate) fn fixed16(&mut self) -> Result<f64> {
        Ok(f64::from(self.i32()?) / 65536.0)
    }

    pub(crate) fn skip_zero_padding(&mut self, limit: usize) {
        let mut skipped = 0;
        while skipped < limit && self.data.get(self.pos) == Some(&0) {
            self.pos += 1;
            skipped += 1;
        }
    }

    pub(crate) fn finish_padded(&self) -> bool {
        self.rest().iter().all(|&byte| byte == 0)
    }
}
