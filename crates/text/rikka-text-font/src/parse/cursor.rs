use crate::parse::array::{Array, Scalar};
use crate::parse::bytes::Bytes;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Cursor<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    #[inline]
    pub(crate) fn from_position(data: &'a [u8], position: usize) -> Self {
        Self { data, position }
    }

    #[inline]
    pub(crate) fn position(&self) -> usize {
        self.position
    }

    #[inline]
    pub(crate) fn skip(&mut self, len: usize) -> Option<()> {
        let end = self.position.checked_add(len)?;
        (end <= self.data.len()).then(|| self.position = end)
    }

    #[inline]
    pub(crate) fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let slice = self.data.range(self.position, len)?;
        self.position += len;
        Some(slice)
    }

    #[inline]
    fn chunk<const N: usize>(&mut self) -> Option<[u8; N]> {
        let bytes = self.data.chunk::<N>(self.position)?;
        self.position += N;
        Some(bytes)
    }

    #[inline]
    pub(crate) fn u8(&mut self) -> Option<u8> {
        self.chunk::<1>().map(|b| b[0])
    }

    #[inline]
    pub(crate) fn i8(&mut self) -> Option<i8> {
        self.chunk::<1>().map(|b| b[0] as i8)
    }

    #[inline]
    pub(crate) fn u16(&mut self) -> Option<u16> {
        self.chunk().map(u16::from_be_bytes)
    }

    #[inline]
    pub(crate) fn i16(&mut self) -> Option<i16> {
        self.chunk().map(i16::from_be_bytes)
    }

    #[inline]
    pub(crate) fn i32(&mut self) -> Option<i32> {
        self.chunk().map(i32::from_be_bytes)
    }

    #[inline]
    pub(crate) fn array<T: Scalar>(&mut self, count: usize) -> Option<Array<'a, T>> {
        let bytes = self.take(count.checked_mul(T::SIZE)?)?;
        Some(Array::new(bytes))
    }
}
