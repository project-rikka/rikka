use crate::layout::device::Device;
use crate::parse::bytes::Bytes;
use crate::parse::f2dot14::F2Dot14;
use crate::variations::item_variation_store::ItemVariationStore;

#[derive(Clone, Copy, Debug)]
pub struct Anchor<'a> {
    data: &'a [u8],
    format: u16,
    x: i16,
    y: i16,
}

impl<'a> Anchor<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        let format = data.u16_at(0)?;
        if !(1..=3).contains(&format) {
            return None;
        }
        Some(Self {
            data,
            format,
            x: data.i16_at(2)?,
            y: data.i16_at(4)?,
        })
    }

    #[inline]
    pub fn x(&self) -> i16 {
        self.x
    }

    #[inline]
    pub fn y(&self) -> i16 {
        self.y
    }

    #[inline]
    pub fn contour_point(&self) -> Option<u16> {
        if self.format == 2 {
            self.data.u16_at(6)
        } else {
            None
        }
    }

    #[inline]
    pub fn x_device(&self) -> Option<Device<'a>> {
        if self.format != 3 {
            return None;
        }
        Device::new(self.data.offset16(0, 6)?)
    }

    #[inline]
    pub fn y_device(&self) -> Option<Device<'a>> {
        if self.format != 3 {
            return None;
        }
        Device::new(self.data.offset16(0, 8)?)
    }

    #[inline]
    pub fn resolve(
        &self,
        coords: &[F2Dot14],
        store: Option<&ItemVariationStore<'_>>,
    ) -> (f32, f32) {
        let mut x = f32::from(self.x);
        let mut y = f32::from(self.y);
        if self.format == 3 && !coords.is_empty() {
            x += self
                .x_device()
                .map_or(0.0, |device| device.variation_delta(coords, store));
            y += self
                .y_device()
                .map_or(0.0, |device| device.variation_delta(coords, store));
        }
        (x, y)
    }
}
