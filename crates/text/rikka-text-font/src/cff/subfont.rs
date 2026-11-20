use crate::cff::dict::{op, parse_dict};
use crate::cff::index::{Index, IndexSpan};

pub(crate) type Matrix = [f64; 6];

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PrivateDict<'a> {
    pub(crate) local_subrs: Index<'a>,
    pub(crate) default_width: f64,
    pub(crate) nominal_width: f64,
    pub(crate) vsindex: u16,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PrivateValues {
    local_subrs: Option<IndexSpan>,
    default_width: f64,
    nominal_width: f64,
    vsindex: u16,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Subfont {
    pub(crate) private: PrivateValues,
    pub(crate) matrix: Option<Matrix>,
}

#[derive(Debug, Default)]
pub(crate) struct Subfonts {
    slots: Vec<Option<Option<Subfont>>>,
}

impl PrivateValues {
    pub(crate) fn parse(table: &[u8], location: Option<(f64, f64)>, is_cff2: bool) -> Option<Self> {
        let Some((size, offset)) = location else {
            return Some(Self::default());
        };
        let size = to_offset(size)?;
        let offset = to_offset(offset)?;
        let dict = table.get(offset..offset.checked_add(size)?)?;
        let mut private = Self::default();
        let mut subrs = None;
        parse_dict(dict, |operator, operands| match operator {
            op::SUBRS => subrs = operands.last().copied().and_then(to_offset),
            op::DEFAULT_WIDTH_X => private.default_width = operands.last().copied().unwrap_or(0.0),
            op::NOMINAL_WIDTH_X => private.nominal_width = operands.last().copied().unwrap_or(0.0),
            op::VSINDEX => private.vsindex = operands.last().map_or(0, |&value| value as u16),
            _ => {}
        })?;
        private.local_subrs = match subrs {
            Some(subrs) => {
                let at = offset.checked_add(subrs)?;
                let span = if is_cff2 {
                    IndexSpan::parse_cff2(table, at)
                } else {
                    IndexSpan::parse_cff(table, at)
                };
                Some(span?.0)
            }
            None => None,
        };
        Some(private)
    }

    #[inline]
    pub(crate) fn bind(self, table: &[u8]) -> Option<PrivateDict<'_>> {
        let local_subrs = match self.local_subrs {
            Some(span) => span.view(table)?,
            None => Index::default(),
        };
        Some(PrivateDict {
            local_subrs,
            default_width: self.default_width,
            nominal_width: self.nominal_width,
            vsindex: self.vsindex,
        })
    }
}

impl Subfont {
    pub(crate) fn parse(table: &[u8], font_dict: &[u8], is_cff2: bool) -> Option<Self> {
        let mut private = None;
        let mut matrix = None;
        parse_dict(font_dict, |operator, operands| match operator {
            op::PRIVATE => {
                if let [size, offset] = operands {
                    private = Some((*size, *offset));
                }
            }
            op::FONT_MATRIX => matrix = matrix_of(operands),
            _ => {}
        })?;
        Some(Self {
            private: PrivateValues::parse(table, private, is_cff2)?,
            matrix,
        })
    }
}

impl Subfonts {
    pub(crate) fn clear(&mut self) {
        self.slots.clear();
    }

    pub(crate) fn get(
        &mut self,
        table: &[u8],
        fd_array: Index<'_>,
        index: u16,
        is_cff2: bool,
    ) -> Option<Subfont> {
        let font_dict = fd_array.get(u32::from(index))?;
        let slot = usize::from(index);
        if slot >= self.slots.len() {
            self.slots.resize(slot + 1, None);
        }
        *self.slots[slot].get_or_insert_with(|| Subfont::parse(table, font_dict, is_cff2))
    }
}

#[inline]
pub(crate) fn to_offset(value: f64) -> Option<usize> {
    (0.0..=f64::from(u32::MAX))
        .contains(&value)
        .then_some(value as usize)
}

pub(crate) fn matrix_of(operands: &[f64]) -> Option<Matrix> {
    operands.try_into().ok()
}

pub(crate) fn transform(
    units_per_em: u16,
    top: Option<Matrix>,
    font: Option<Matrix>,
) -> Option<[f32; 4]> {
    let [a, b, c, d, ..] = match (top, font) {
        (Some(t), Some(f)) => [
            t[0] * f[0] + t[2] * f[1],
            t[1] * f[0] + t[3] * f[1],
            t[0] * f[2] + t[2] * f[3],
            t[1] * f[2] + t[3] * f[3],
            0.0,
            0.0,
        ],
        (Some(m), None) | (None, Some(m)) => m,
        (None, None) => return None,
    };
    let scale = f64::from(units_per_em);
    let scaled = [a * scale, b * scale, c * scale, d * scale];
    let identity = [1.0, 0.0, 0.0, 1.0];
    if scaled
        .iter()
        .zip(identity)
        .all(|(value, expected)| (value - expected).abs() < 1e-6)
    {
        return None;
    }
    if !scaled.iter().all(|value| value.is_finite()) {
        return None;
    }
    Some(scaled.map(|value| value as f32))
}
