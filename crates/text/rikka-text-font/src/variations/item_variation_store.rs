use crate::parse::array::Array;
use crate::parse::bytes::Bytes;
use crate::parse::f2dot14::F2Dot14;
use crate::variations::fixed::Fixed;

const LONG_WORDS: u16 = 0x8000;
const WORD_COUNT_MASK: u16 = 0x7FFF;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DeltaSetIndex {
    pub outer: u16,
    pub inner: u16,
}

impl DeltaSetIndex {
    pub const NO_VARIATIONS: u32 = 0xFFFF_FFFF;

    #[inline]
    pub fn from_packed(index: u32) -> Self {
        Self {
            outer: (index >> 16) as u16,
            inner: index as u16,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ItemVariationStore<'a> {
    data: &'a [u8],
    regions: RegionList<'a>,
    data_offsets: Array<'a, u32>,
}

#[derive(Clone, Debug, Default)]
pub struct RegionScalars {
    sets: Box<[ScaledRows]>,
    columns: Box<[ScaledColumn]>,
}

#[derive(Clone, Copy, Debug, Default)]
struct ScaledRows {
    rows: usize,
    row_size: usize,
    item_count: u16,
    long_words: bool,
    start: usize,
    words_end: usize,
    end: usize,
}

#[derive(Clone, Copy, Debug)]
struct ScaledColumn {
    offset: usize,
    scalar: i32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RegionList<'a> {
    axis_count: usize,
    region_count: usize,
    records: &'a [u8],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ItemVariationData<'a> {
    item_count: u16,
    word_count: usize,
    long_words: bool,
    region_indices: Array<'a, u16>,
    rows: &'a [u8],
    row_size: usize,
}

impl<'a> ItemVariationStore<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        if data.u16_at(0)? != 1 {
            return None;
        }
        let regions = RegionList::new(data.offset32(0, 2)?)?;
        let data_count = data.u16_at(6)?;
        let data_offsets = data.array::<u32>(8, usize::from(data_count))?;
        Some(Self {
            data,
            regions,
            data_offsets,
        })
    }

    pub(crate) fn regions(&self) -> RegionList<'a> {
        self.regions
    }

    pub(crate) fn variation_data(&self, outer: u16) -> Option<ItemVariationData<'a>> {
        let offset = self.data_offsets.get(usize::from(outer))?;
        ItemVariationData::new(self.data.tail(offset as usize)?)
    }

    pub fn delta(&self, index: DeltaSetIndex, coords: &[F2Dot14]) -> f32 {
        if coords.iter().all(|c| c.0 == 0) {
            return 0.0;
        }
        self.try_delta(index, coords).unwrap_or(0.0)
    }

    pub fn fixed_scalars(&self, coords: &[F2Dot14]) -> RegionScalars {
        let scalars: Vec<i32> = (0..self.regions.region_count)
            .map(|region| self.regions.fixed_scalar(region, coords).0)
            .collect();
        let mut columns = Vec::new();
        let sets = (0..self.data_offsets.len())
            .map(|outer| self.scaled_rows(outer, &scalars, &mut columns))
            .collect();
        RegionScalars {
            sets,
            columns: columns.into_boxed_slice(),
        }
    }

    fn scaled_rows(
        &self,
        outer: usize,
        scalars: &[i32],
        columns: &mut Vec<ScaledColumn>,
    ) -> ScaledRows {
        let start = columns.len();
        let Some((rows, data)) = u16::try_from(outer).ok().and_then(|outer| {
            let offset = self.data_offsets.get(usize::from(outer))? as usize;
            let data = self.variation_data(outer)?;
            Some((offset + 6 + data.region_indices.len() * 2, data))
        }) else {
            return ScaledRows::default();
        };
        let (word_size, short_size) = if data.long_words { (4, 2) } else { (2, 1) };
        let mut words_end = start;
        for (column, region) in data.region_indices.iter().enumerate() {
            let Some(&scalar) = scalars.get(usize::from(region)) else {
                columns.truncate(start);
                return ScaledRows::default();
            };
            if scalar == 0 {
                continue;
            }
            let offset = if column < data.word_count {
                column * word_size
            } else {
                data.word_count * word_size + (column - data.word_count) * short_size
            };
            columns.push(ScaledColumn { offset, scalar });
            if column < data.word_count {
                words_end = columns.len();
            }
        }
        ScaledRows {
            rows,
            row_size: data.row_size,
            item_count: data.item_count,
            long_words: data.long_words,
            start,
            words_end,
            end: columns.len(),
        }
    }

    #[inline]
    pub fn delta_fixed(&self, scalars: &RegionScalars, index: DeltaSetIndex) -> i32 {
        let Some(set) = scalars.sets.get(usize::from(index.outer)) else {
            return 0;
        };
        if index.inner >= set.item_count {
            return 0;
        }
        let start = set.rows + usize::from(index.inner) * set.row_size;
        let Some(row) = self.data.range(start, set.row_size) else {
            return 0;
        };
        let words = &scalars.columns[set.start..set.words_end];
        let shorts = &scalars.columns[set.words_end..set.end];
        let total = if set.long_words {
            scaled_sum(row, words, |row, at| row.i32_at(at))
                + scaled_sum(row, shorts, |row, at| row.i16_at(at).map(i32::from))
        } else {
            scaled_sum(row, words, |row, at| row.i16_at(at).map(i32::from))
                + scaled_sum(row, shorts, |row, at| row.i8_at(at).map(i32::from))
        };
        ((total + 0x8000) >> 16) as i32
    }

    fn try_delta(&self, index: DeltaSetIndex, coords: &[F2Dot14]) -> Option<f32> {
        let data = self.variation_data(index.outer)?;
        let mut total = 0.0;
        for (column, region) in data.region_indices.iter().enumerate() {
            let scalar = self.regions.scalar(usize::from(region), coords);
            if scalar != 0.0 {
                total += scalar * data.delta(index.inner, column)? as f32;
            }
        }
        Some(total)
    }
}

impl<'a> RegionList<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        let axis_count = usize::from(data.u16_at(0)?);
        let region_count = usize::from(data.u16_at(2)?);
        let records = data.range(4, axis_count * region_count * 6)?;
        Some(Self {
            axis_count,
            region_count,
            records,
        })
    }

    pub(crate) fn scalar(&self, region: usize, coords: &[F2Dot14]) -> f32 {
        if region >= self.region_count {
            return 0.0;
        }
        let base = region * self.axis_count * 6;
        let mut scalar = 1.0;
        for axis in 0..self.axis_count {
            let record = base + axis * 6;
            let (Some(start), Some(peak), Some(end)) = (
                self.records.i16_at(record),
                self.records.i16_at(record + 2),
                self.records.i16_at(record + 4),
            ) else {
                return 0.0;
            };
            let coord = coords.get(axis).map_or(0, |c| c.0);
            let factor = axis_scalar(start, peak, end, coord);
            if factor == 0.0 {
                return 0.0;
            }
            scalar *= factor;
        }
        scalar
    }

    fn fixed_scalar(&self, region: usize, coords: &[F2Dot14]) -> Fixed {
        let base = region * self.axis_count * 6;
        let mut scalar = Fixed::ONE;
        for axis in 0..self.axis_count {
            let record = base + axis * 6;
            let (Some(start), Some(peak), Some(end)) = (
                self.records.i16_at(record),
                self.records.i16_at(record + 2),
                self.records.i16_at(record + 4),
            ) else {
                return Fixed::ZERO;
            };
            if peak == 0 || start > peak || peak > end || (start < 0 && end > 0) {
                continue;
            }
            let coord = coords.get(axis).map_or(0, |c| c.0);
            if coord < start || coord > end {
                return Fixed::ZERO;
            }
            if coord == peak {
                continue;
            }
            let [start, peak, end, coord] = [start, peak, end, coord].map(Fixed::from_f2dot14);
            scalar = if coord.0 < peak.0 {
                scalar.mul_div(coord.sub(start), peak.sub(start))
            } else {
                scalar.mul_div(end.sub(coord), end.sub(peak))
            };
        }
        scalar
    }
}

#[inline]
pub(crate) fn axis_scalar(start: i16, peak: i16, end: i16, coord: i16) -> f32 {
    if start > peak || peak > end || peak == 0 || (start < 0 && end > 0) {
        return 1.0;
    }
    if coord < start || coord > end {
        return 0.0;
    }
    if coord == peak {
        return 1.0;
    }
    let (coord, start, peak, end) = (
        f32::from(coord),
        f32::from(start),
        f32::from(peak),
        f32::from(end),
    );
    if coord < peak {
        (coord - start) / (peak - start)
    } else {
        (end - coord) / (end - peak)
    }
}

impl<'a> ItemVariationData<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        let item_count = data.u16_at(0)?;
        let word_delta_count = data.u16_at(2)?;
        let region_count = usize::from(data.u16_at(4)?);
        let region_indices = data.array::<u16>(6, region_count)?;
        let long_words = word_delta_count & LONG_WORDS != 0;
        let word_count = usize::from(word_delta_count & WORD_COUNT_MASK).min(region_count);
        let (word_size, short_size) = if long_words { (4, 2) } else { (2, 1) };
        let row_size = word_count * word_size + (region_count - word_count) * short_size;
        let rows = data.range(6 + region_count * 2, usize::from(item_count) * row_size)?;
        Some(Self {
            item_count,
            word_count,
            long_words,
            region_indices,
            rows,
            row_size,
        })
    }

    #[inline]
    pub(crate) fn region_indices(&self) -> Array<'a, u16> {
        self.region_indices
    }

    #[inline]
    pub(crate) fn delta(&self, inner: u16, column: usize) -> Option<i32> {
        if inner >= self.item_count {
            return None;
        }
        let row = usize::from(inner) * self.row_size;
        if self.long_words {
            if column < self.word_count {
                self.rows.i32_at(row + column * 4)
            } else {
                let offset = self.word_count * 4 + (column - self.word_count) * 2;
                self.rows.i16_at(row + offset).map(i32::from)
            }
        } else if column < self.word_count {
            self.rows.i16_at(row + column * 2).map(i32::from)
        } else {
            let offset = self.word_count * 2 + (column - self.word_count);
            self.rows.i8_at(row + offset).map(i32::from)
        }
    }
}

#[inline]
fn scaled_sum(
    row: &[u8],
    columns: &[ScaledColumn],
    read: impl Fn(&[u8], usize) -> Option<i32>,
) -> i64 {
    columns
        .iter()
        .map(|column| i64::from(read(row, column.offset).unwrap_or(0)) * i64::from(column.scalar))
        .sum()
}
