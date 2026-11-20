use crate::layout::device::Device;
use crate::parse::bytes::Bytes;
use crate::parse::f2dot14::F2Dot14;
use crate::variations::item_variation_store::ItemVariationStore;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ValueFormat(pub u16);

impl ValueFormat {
    pub const X_PLACEMENT: u16 = 0x0001;
    pub const Y_PLACEMENT: u16 = 0x0002;
    pub const X_ADVANCE: u16 = 0x0004;
    pub const Y_ADVANCE: u16 = 0x0008;
    pub const X_PLACEMENT_DEVICE: u16 = 0x0010;
    pub const Y_PLACEMENT_DEVICE: u16 = 0x0020;
    pub const X_ADVANCE_DEVICE: u16 = 0x0040;
    pub const Y_ADVANCE_DEVICE: u16 = 0x0080;

    #[inline]
    pub fn size(self) -> usize {
        (self.0 & 0x00FF).count_ones() as usize * 2
    }

    #[inline]
    pub fn is_empty(self) -> bool {
        self.0 & 0x00FF == 0
    }

    #[inline]
    pub fn contains(self, field: u16) -> bool {
        self.0 & field != 0
    }

    #[inline]
    fn offset_of(self, field: u16) -> Option<usize> {
        self.contains(field)
            .then(|| (self.0 & (field - 1)).count_ones() as usize * 2)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ValueRecord<'a> {
    base: &'a [u8],
    at: u32,
    format: ValueFormat,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PositionAdjustment {
    pub x_placement: f32,
    pub y_placement: f32,
    pub x_advance: f32,
    pub y_advance: f32,
}

impl<'a> ValueRecord<'a> {
    #[inline]
    pub(crate) fn new(format: ValueFormat, base: &'a [u8], at: usize) -> Option<Self> {
        base.range(at, format.size())?;
        Some(Self {
            base,
            at: u32::try_from(at).ok()?,
            format,
        })
    }

    #[inline]
    pub fn format(&self) -> ValueFormat {
        self.format
    }

    #[inline]
    fn field(&self, field: u16) -> Option<u16> {
        self.base
            .u16_at(self.at as usize + self.format.offset_of(field)?)
    }

    #[inline]
    fn device(&self, field: u16) -> Option<Device<'a>> {
        let offset = self.field(field)?;
        if offset == 0 {
            return None;
        }
        Device::new(self.base.tail(usize::from(offset))?)
    }

    #[inline]
    pub fn x_placement(&self) -> i16 {
        self.field(ValueFormat::X_PLACEMENT).unwrap_or(0) as i16
    }

    #[inline]
    pub fn y_placement(&self) -> i16 {
        self.field(ValueFormat::Y_PLACEMENT).unwrap_or(0) as i16
    }

    #[inline]
    pub fn x_advance(&self) -> i16 {
        self.field(ValueFormat::X_ADVANCE).unwrap_or(0) as i16
    }

    #[inline]
    pub fn y_advance(&self) -> i16 {
        self.field(ValueFormat::Y_ADVANCE).unwrap_or(0) as i16
    }

    #[inline]
    pub fn x_placement_device(&self) -> Option<Device<'a>> {
        self.device(ValueFormat::X_PLACEMENT_DEVICE)
    }

    #[inline]
    pub fn y_placement_device(&self) -> Option<Device<'a>> {
        self.device(ValueFormat::Y_PLACEMENT_DEVICE)
    }

    #[inline]
    pub fn x_advance_device(&self) -> Option<Device<'a>> {
        self.device(ValueFormat::X_ADVANCE_DEVICE)
    }

    #[inline]
    pub fn y_advance_device(&self) -> Option<Device<'a>> {
        self.device(ValueFormat::Y_ADVANCE_DEVICE)
    }

    pub fn resolve(
        &self,
        coords: &[F2Dot14],
        store: Option<&ItemVariationStore<'_>>,
    ) -> PositionAdjustment {
        let mut adjustment = PositionAdjustment {
            x_placement: f32::from(self.x_placement()),
            y_placement: f32::from(self.y_placement()),
            x_advance: f32::from(self.x_advance()),
            y_advance: f32::from(self.y_advance()),
        };
        if coords.is_empty() || store.is_none() || self.format.0 & 0x00F0 == 0 {
            return adjustment;
        }
        let delta = |device: Option<Device<'_>>| {
            device.map_or(0.0, |device| device.variation_delta(coords, store))
        };
        adjustment.x_placement += delta(self.x_placement_device());
        adjustment.y_placement += delta(self.y_placement_device());
        adjustment.x_advance += delta(self.x_advance_device());
        adjustment.y_advance += delta(self.y_advance_device());
        adjustment
    }
}
