use crate::name::name_id::NameId;
use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::table_id::TableId;
use crate::sfnt::tag::Tag;

const HIDDEN_AXIS: u16 = 0x0001;
const AXIS_RECORD_SIZE: usize = 20;

#[derive(Clone, Copy, Debug)]
pub struct Fvar<'a> {
    axes: &'a [u8],
    axis_count: u16,
    axis_size: usize,
    instances: &'a [u8],
    instance_count: u16,
    instance_size: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VariationAxis {
    pub tag: Tag,
    pub min: f32,
    pub default: f32,
    pub max: f32,
    pub hidden: bool,
    pub name_id: NameId,
}

impl VariationAxis {
    pub fn normalize(&self, value: f32) -> f32 {
        let value = value.clamp(self.min, self.max);
        if value < self.default {
            (value - self.default) / (self.default - self.min)
        } else if value > self.default {
            (value - self.default) / (self.max - self.default)
        } else {
            0.0
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NamedInstance<'a> {
    pub subfamily_name_id: NameId,
    pub postscript_name_id: Option<NameId>,
    coordinates: &'a [u8],
}

impl<'a> NamedInstance<'a> {
    pub fn coordinates(&self) -> impl ExactSizeIterator<Item = f32> + use<'a> {
        let coordinates = self.coordinates;
        (0..coordinates.len() / 4).map(move |axis| coordinates.fixed_at(axis * 4).unwrap_or(0.0))
    }
}

impl<'a> Fvar<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        if data.u16_at(0)? != 1 {
            return None;
        }
        let axes_offset = usize::from(data.u16_at(4)?);
        let axis_count = data.u16_at(8)?;
        let axis_size = usize::from(data.u16_at(10)?);
        let instance_count = data.u16_at(12)?;
        let instance_size = usize::from(data.u16_at(14)?);
        if axis_size < AXIS_RECORD_SIZE {
            return None;
        }
        let axes_len = usize::from(axis_count) * axis_size;
        let axes = data.range(axes_offset, axes_len)?;
        let instances = if instance_size >= usize::from(axis_count) * 4 + 4 {
            data.range(
                axes_offset + axes_len,
                usize::from(instance_count) * instance_size,
            )
        } else {
            None
        };
        Some(Self {
            axes,
            axis_count,
            axis_size,
            instances: instances.unwrap_or_default(),
            instance_count: if instances.is_some() {
                instance_count
            } else {
                0
            },
            instance_size,
        })
    }

    #[inline]
    pub fn axis_count(&self) -> u16 {
        self.axis_count
    }

    pub fn axis(&self, index: u16) -> Option<VariationAxis> {
        if index >= self.axis_count {
            return None;
        }
        let record = self
            .axes
            .range(usize::from(index) * self.axis_size, AXIS_RECORD_SIZE)?;
        let min = record.fixed_at(4)?;
        let default = record.fixed_at(8)?;
        let max = record.fixed_at(12)?;
        Some(VariationAxis {
            tag: record.tag_at(0)?,
            min: min.min(default),
            default,
            max: max.max(default),
            hidden: record.u16_at(16)? & HIDDEN_AXIS != 0,
            name_id: NameId(record.u16_at(18)?),
        })
    }

    pub fn axes(&self) -> impl Iterator<Item = VariationAxis> + use<'a> {
        let fvar = *self;
        (0..self.axis_count).filter_map(move |index| fvar.axis(index))
    }

    #[inline]
    pub fn instance_count(&self) -> u16 {
        self.instance_count
    }

    pub fn instance(&self, index: u16) -> Option<NamedInstance<'a>> {
        if index >= self.instance_count {
            return None;
        }
        let record = self
            .instances
            .range(usize::from(index) * self.instance_size, self.instance_size)?;
        let coordinates_len = usize::from(self.axis_count) * 4;
        let postscript_offset = 4 + coordinates_len;
        let postscript_name_id = if self.instance_size >= postscript_offset + 2 {
            record
                .u16_at(postscript_offset)
                .filter(|&id| id != 0xFFFF)
                .map(NameId)
        } else {
            None
        };
        Some(NamedInstance {
            subfamily_name_id: NameId(record.u16_at(0)?),
            postscript_name_id,
            coordinates: record.range(4, coordinates_len)?,
        })
    }

    pub fn instances(&self) -> impl Iterator<Item = NamedInstance<'a>> + use<'a> {
        let fvar = *self;
        (0..self.instance_count).filter_map(move |index| fvar.instance(index))
    }
}

impl<'a> Font<'a> {
    pub fn fvar(&self) -> Option<Fvar<'a>> {
        Fvar::new(self.table_data(TableId::Fvar)?)
    }

    pub fn is_variable(&self) -> bool {
        self.fvar().is_some_and(|fvar| fvar.axis_count() > 0)
    }
}
