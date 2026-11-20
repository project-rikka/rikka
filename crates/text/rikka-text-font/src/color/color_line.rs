use crate::color::color_value::ColorIndex;
use crate::color::colr_variations::ColrVariations;
use crate::parse::bytes::Bytes;

const STOP_SIZE: usize = 6;
const VAR_STOP_SIZE: usize = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Extend {
    #[default]
    Pad,
    Repeat,
    Reflect,
}

impl Extend {
    fn from_raw(raw: u8) -> Self {
        match raw {
            1 => Self::Repeat,
            2 => Self::Reflect,
            _ => Self::Pad,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorStop {
    pub offset: f32,
    pub color: ColorIndex,
    pub alpha: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ColorStops<'c> {
    records: &'c [u8],
    record_size: usize,
    variations: ColrVariations<'c>,
}

impl<'c> ColorStops<'c> {
    pub(crate) fn parse(
        data: &'c [u8],
        variable: bool,
        variations: ColrVariations<'c>,
    ) -> Option<(Self, Extend)> {
        let extend = Extend::from_raw(data.u8_at(0)?);
        let count = usize::from(data.u16_at(1)?);
        let record_size = if variable { VAR_STOP_SIZE } else { STOP_SIZE };
        let records = data.range(3, count * record_size)?;
        Some((
            Self {
                records,
                record_size,
                variations,
            },
            extend,
        ))
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.records.len() / self.record_size
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<ColorStop> {
        let record = self
            .records
            .range(index * self.record_size, self.record_size)?;
        let offset = record.i16_at(0)?;
        let color = ColorIndex::from_raw(record.u16_at(2)?);
        let alpha = record.i16_at(4)?;
        let base = if self.record_size == VAR_STOP_SIZE {
            record.u32_at(6)?
        } else {
            u32::MAX
        };
        Some(ColorStop {
            offset: self.variations.f2dot14(offset, base, 0),
            color,
            alpha: self.variations.f2dot14(alpha, base, 1),
        })
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = ColorStop> + use<'c> {
        let stops = *self;
        (0..self.len()).map(move |index| {
            stops.get(index).unwrap_or(ColorStop {
                offset: 0.0,
                color: ColorIndex::Foreground,
                alpha: 0.0,
            })
        })
    }

    pub(crate) fn lowest(&self) -> Option<ColorStop> {
        self.iter().reduce(|lowest, stop| {
            if stop.offset < lowest.offset {
                stop
            } else {
                lowest
            }
        })
    }
}
