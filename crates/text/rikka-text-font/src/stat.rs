use crate::name::name_id::NameId;
use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::table_id::TableId;
use crate::sfnt::tag::Tag;

const OLDER_SIBLING_FONT_ATTRIBUTE: u16 = 0x0001;
const ELIDABLE_AXIS_VALUE_NAME: u16 = 0x0002;
const AXIS_RECORD_SIZE: usize = 8;

#[derive(Clone, Copy, Debug)]
pub struct Stat<'a> {
    axes: &'a [u8],
    axis_size: usize,
    value_offsets: &'a [u8],
    value_count: usize,
    elided_fallback_name_id: Option<NameId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DesignAxis {
    pub tag: Tag,
    pub name_id: NameId,
    pub ordering: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct AxisValue<'a> {
    pub flags: u16,
    pub name_id: NameId,
    pub kind: AxisValueKind<'a>,
}

#[derive(Clone, Copy, Debug)]
pub enum AxisValueKind<'a> {
    Single {
        axis_index: u16,
        value: f32,
    },
    Range {
        axis_index: u16,
        nominal: f32,
        min: f32,
        max: f32,
    },
    Linked {
        axis_index: u16,
        value: f32,
        linked: f32,
    },
    Combination(AxisCombination<'a>),
}

#[derive(Clone, Copy, Debug)]
pub struct AxisCombination<'a> {
    records: &'a [u8],
}

impl<'a> AxisCombination<'a> {
    pub fn len(&self) -> usize {
        self.records.len() / 6
    }

    pub fn is_empty(&self) -> bool {
        self.records.len() < 6
    }

    pub fn iter(&self) -> impl Iterator<Item = (u16, f32)> + use<'a> {
        let records = self.records;
        (0..records.len() / 6).filter_map(move |index| {
            Some((records.u16_at(index * 6)?, records.fixed_at(index * 6 + 2)?))
        })
    }
}

impl AxisValue<'_> {
    pub fn is_elidable(&self) -> bool {
        self.flags & ELIDABLE_AXIS_VALUE_NAME != 0
    }

    pub fn is_older_sibling_attribute(&self) -> bool {
        self.flags & OLDER_SIBLING_FONT_ATTRIBUTE != 0
    }
}

impl<'a> Stat<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        if data.u16_at(0)? != 1 {
            return None;
        }
        let minor = data.u16_at(2)?;
        let axis_size = usize::from(data.u16_at(4)?);
        let axis_count = usize::from(data.u16_at(6)?);
        let axes = if axis_count == 0 || axis_size < AXIS_RECORD_SIZE {
            &[]
        } else {
            data.offset32(0, 8)?.range(0, axis_count * axis_size)?
        };
        let value_count = usize::from(data.u16_at(12)?);
        let value_offsets = if value_count == 0 {
            &[]
        } else {
            let base = data.offset32(0, 14)?;
            base.range(0, value_count * 2)?;
            base
        };
        Some(Self {
            axes,
            axis_size: axis_size.max(AXIS_RECORD_SIZE),
            value_offsets,
            value_count,
            elided_fallback_name_id: if minor >= 1 {
                data.u16_at(18).map(NameId)
            } else {
                None
            },
        })
    }

    pub fn elided_fallback_name_id(&self) -> Option<NameId> {
        self.elided_fallback_name_id
    }

    pub fn design_axes(&self) -> impl Iterator<Item = DesignAxis> + use<'a> {
        let (axes, size) = (self.axes, self.axis_size);
        (0..axes.len() / size).filter_map(move |index| {
            let record = index * size;
            Some(DesignAxis {
                tag: axes.tag_at(record)?,
                name_id: NameId(axes.u16_at(record + 4)?),
                ordering: axes.u16_at(record + 6)?,
            })
        })
    }

    pub fn axis_values(&self) -> impl Iterator<Item = AxisValue<'a>> + use<'a> {
        let offsets = self.value_offsets;
        (0..self.value_count)
            .filter_map(move |index| parse_axis_value(offsets.offset16(0, index * 2)?))
    }
}

fn parse_axis_value(data: &[u8]) -> Option<AxisValue<'_>> {
    let format = data.u16_at(0)?;
    let (flags, name_id, kind) = match format {
        1 => (
            data.u16_at(4)?,
            data.u16_at(6)?,
            AxisValueKind::Single {
                axis_index: data.u16_at(2)?,
                value: data.fixed_at(8)?,
            },
        ),
        2 => (
            data.u16_at(4)?,
            data.u16_at(6)?,
            AxisValueKind::Range {
                axis_index: data.u16_at(2)?,
                nominal: data.fixed_at(8)?,
                min: data.fixed_at(12)?,
                max: data.fixed_at(16)?,
            },
        ),
        3 => (
            data.u16_at(4)?,
            data.u16_at(6)?,
            AxisValueKind::Linked {
                axis_index: data.u16_at(2)?,
                value: data.fixed_at(8)?,
                linked: data.fixed_at(12)?,
            },
        ),
        4 => {
            let count = usize::from(data.u16_at(2)?);
            (
                data.u16_at(4)?,
                data.u16_at(6)?,
                AxisValueKind::Combination(AxisCombination {
                    records: data.range(8, count * 6)?,
                }),
            )
        }
        _ => return None,
    };
    Some(AxisValue {
        flags,
        name_id: NameId(name_id),
        kind,
    })
}

impl<'a> Font<'a> {
    pub fn stat(&self) -> Option<Stat<'a>> {
        Stat::new(self.table_data(TableId::Stat)?)
    }
}
