use crate::layout::gpos::anchor::Anchor;
use crate::parse::bytes::Bytes;

#[derive(Clone, Copy, Debug)]
pub struct MarkArray<'a> {
    data: &'a [u8],
    count: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct MarkRecord<'a> {
    pub class: u16,
    pub anchor: Anchor<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct AnchorMatrix<'a> {
    data: &'a [u8],
    rows: usize,
    columns: usize,
}

impl<'a> MarkArray<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        let count = usize::from(data.u16_at(0)?);
        data.range(2, count * 4)?;
        Some(Self { data, count })
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.count
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    #[inline]
    pub fn get(&self, index: u16) -> Option<MarkRecord<'a>> {
        let index = usize::from(index);
        if index >= self.count {
            return None;
        }
        let record = 2 + index * 4;
        Some(MarkRecord {
            class: self.data.u16_at(record)?,
            anchor: Anchor::new(self.data.offset16(0, record + 2)?)?,
        })
    }
}

impl<'a> AnchorMatrix<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8], columns: u16) -> Option<Self> {
        let rows = usize::from(data.u16_at(0)?);
        let columns = usize::from(columns);
        data.range(2, rows.checked_mul(columns)?.checked_mul(2)?)?;
        Some(Self {
            data,
            rows,
            columns,
        })
    }

    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    #[inline]
    pub fn columns(&self) -> usize {
        self.columns
    }

    #[inline]
    pub fn anchor(&self, row: usize, column: u16) -> Option<Anchor<'a>> {
        let column = usize::from(column);
        if row >= self.rows || column >= self.columns {
            return None;
        }
        Anchor::new(
            self.data
                .offset16(0, 2 + (row * self.columns + column) * 2)?,
        )
    }
}
