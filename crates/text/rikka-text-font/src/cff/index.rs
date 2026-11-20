use crate::parse::bytes::Bytes;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Index<'a> {
    count: u32,
    offset_size: usize,
    offsets: &'a [u8],
    data: &'a [u8],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct IndexSpan {
    count: u32,
    offset_size: usize,
    offsets_at: usize,
    data_at: usize,
    data_len: usize,
}

impl<'a> Index<'a> {
    pub(crate) fn parse_cff(data: &'a [u8], at: usize) -> Option<(Self, usize)> {
        let (span, end) = IndexSpan::parse_cff(data, at)?;
        Some((span.view(data)?, end))
    }

    #[inline]
    fn offset(&self, index: usize) -> Option<usize> {
        let start = index * self.offset_size;
        let bytes = self.offsets.range(start, self.offset_size)?;
        Some(
            bytes
                .iter()
                .fold(0usize, |value, &byte| (value << 8) | usize::from(byte)),
        )
    }

    #[inline]
    pub(crate) fn len(&self) -> u32 {
        self.count
    }

    #[inline]
    pub(crate) fn get(&self, index: u32) -> Option<&'a [u8]> {
        if index >= self.count {
            return None;
        }
        let start = self.offset(index as usize)?.checked_sub(1)?;
        let end = self.offset(index as usize + 1)?.checked_sub(1)?;
        self.data.get(start..end)
    }

    #[inline]
    pub(crate) fn subr_bias(&self) -> i32 {
        if self.count < 1240 {
            107
        } else if self.count < 33900 {
            1131
        } else {
            32768
        }
    }
}

impl IndexSpan {
    pub(crate) fn parse_cff(data: &[u8], at: usize) -> Option<(Self, usize)> {
        let count = u32::from(data.u16_at(at)?);
        Self::parse_body(data, at + 2, count)
    }

    pub(crate) fn parse_cff2(data: &[u8], at: usize) -> Option<(Self, usize)> {
        let count = data.u32_at(at)?;
        Self::parse_body(data, at + 4, count)
    }

    fn parse_body(data: &[u8], at: usize, count: u32) -> Option<(Self, usize)> {
        if count == 0 {
            return Some((Self::default(), at));
        }
        let offset_size = usize::from(data.u8_at(at)?);
        if !(1..=4).contains(&offset_size) {
            return None;
        }
        let offsets_len = (count as usize).checked_add(1)?.checked_mul(offset_size)?;
        let offsets_at = at + 1;
        let offsets = data.range(offsets_at, offsets_len)?;
        let data_at = offsets_at + offsets_len;
        let probe = Index {
            count,
            offset_size,
            offsets,
            data: &[],
        };
        let data_len = probe.offset(count as usize)?.checked_sub(1)?;
        data.range(data_at, data_len)?;
        let span = Self {
            count,
            offset_size,
            offsets_at,
            data_at,
            data_len,
        };
        Some((span, data_at + data_len))
    }

    #[inline]
    pub(crate) fn view<'a>(&self, data: &'a [u8]) -> Option<Index<'a>> {
        if self.count == 0 {
            return Some(Index::default());
        }
        Some(Index {
            count: self.count,
            offset_size: self.offset_size,
            offsets: data.range(
                self.offsets_at,
                (self.count as usize + 1) * self.offset_size,
            )?,
            data: data.range(self.data_at, self.data_len)?,
        })
    }
}
