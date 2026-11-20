use crate::parse::array::{Array, Scalar};
use crate::sfnt::tag::Tag;

pub(crate) trait Bytes<'a>: Sized + Copy {
    fn chunk<const N: usize>(self, offset: usize) -> Option<[u8; N]>;
    fn range(self, offset: usize, len: usize) -> Option<&'a [u8]>;
    fn tail(self, offset: usize) -> Option<&'a [u8]>;

    #[inline]
    fn u8_at(self, offset: usize) -> Option<u8> {
        self.chunk::<1>(offset).map(|b| b[0])
    }

    #[inline]
    fn i8_at(self, offset: usize) -> Option<i8> {
        self.chunk::<1>(offset).map(|b| b[0] as i8)
    }

    #[inline]
    fn u16_at(self, offset: usize) -> Option<u16> {
        self.chunk(offset).map(u16::from_be_bytes)
    }

    #[inline]
    fn i16_at(self, offset: usize) -> Option<i16> {
        self.chunk(offset).map(i16::from_be_bytes)
    }

    #[inline]
    fn u24_at(self, offset: usize) -> Option<u32> {
        self.chunk::<3>(offset)
            .map(|b| u32::from_be_bytes([0, b[0], b[1], b[2]]))
    }

    #[inline]
    fn u32_at(self, offset: usize) -> Option<u32> {
        self.chunk(offset).map(u32::from_be_bytes)
    }

    #[inline]
    fn i32_at(self, offset: usize) -> Option<i32> {
        self.chunk(offset).map(i32::from_be_bytes)
    }

    #[inline]
    fn tag_at(self, offset: usize) -> Option<Tag> {
        self.u32_at(offset).map(Tag::from_u32)
    }

    #[inline]
    fn fixed_at(self, offset: usize) -> Option<f32> {
        self.i32_at(offset).map(|v| v as f32 / 65536.0)
    }

    #[inline]
    fn array<T: Scalar>(self, offset: usize, count: usize) -> Option<Array<'a, T>> {
        self.range(offset, count.checked_mul(T::SIZE)?)
            .map(Array::new)
    }

    #[inline]
    fn offset16(self, base: usize, at: usize) -> Option<&'a [u8]> {
        let offset = self.u16_at(at)?;
        if offset == 0 {
            return None;
        }
        self.tail(base.checked_add(usize::from(offset))?)
    }

    #[inline]
    fn offset32(self, base: usize, at: usize) -> Option<&'a [u8]> {
        let offset = self.u32_at(at)?;
        if offset == 0 {
            return None;
        }
        self.tail(base.checked_add(offset as usize)?)
    }
}

impl<'a> Bytes<'a> for &'a [u8] {
    #[inline]
    fn chunk<const N: usize>(self, offset: usize) -> Option<[u8; N]> {
        self.get(offset..)?.first_chunk::<N>().copied()
    }

    #[inline]
    fn range(self, offset: usize, len: usize) -> Option<&'a [u8]> {
        self.get(offset..offset.checked_add(len)?)
    }

    #[inline]
    fn tail(self, offset: usize) -> Option<&'a [u8]> {
        self.get(offset..)
    }
}
