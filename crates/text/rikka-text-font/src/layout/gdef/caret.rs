use crate::layout::device::Device;
use crate::layout::offset_list::OffsetList;
use crate::parse::bytes::Bytes;
use crate::parse::f2dot14::F2Dot14;
use crate::variations::item_variation_store::ItemVariationStore;

#[derive(Clone, Copy, Debug)]
pub enum CaretValue<'a> {
    Coordinate(i16),
    ContourPoint(u16),
    DeviceCoordinate(i16, Option<Device<'a>>),
}

#[derive(Clone, Copy, Debug)]
pub struct LigatureCarets<'a> {
    carets: OffsetList<'a>,
}

impl<'a> CaretValue<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        Some(match data.u16_at(0)? {
            1 => Self::Coordinate(data.i16_at(2)?),
            2 => Self::ContourPoint(data.u16_at(2)?),
            3 => Self::DeviceCoordinate(data.i16_at(2)?, data.offset16(0, 4).and_then(Device::new)),
            _ => return None,
        })
    }

    pub fn coordinate(
        &self,
        coords: &[F2Dot14],
        store: Option<&ItemVariationStore<'_>>,
    ) -> Option<f32> {
        match self {
            Self::Coordinate(x) => Some(f32::from(*x)),
            Self::ContourPoint(_) => None,
            Self::DeviceCoordinate(x, device) => Some(
                f32::from(*x) + device.map_or(0.0, |device| device.variation_delta(coords, store)),
            ),
        }
    }
}

impl<'a> LigatureCarets<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        Some(Self {
            carets: OffsetList::new(data, 0)?,
        })
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.carets.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.carets.len() == 0
    }

    pub fn get(&self, index: usize) -> Option<CaretValue<'a>> {
        CaretValue::new(self.carets.get(index)?)
    }

    pub fn iter(&self) -> impl Iterator<Item = CaretValue<'a>> + use<'a> {
        let carets = *self;
        (0..self.len()).filter_map(move |index| carets.get(index))
    }
}
