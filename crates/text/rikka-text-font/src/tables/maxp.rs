use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::table_id::TableId;

#[derive(Clone, Copy, Debug)]
pub struct Maxp<'a> {
    data: &'a [u8],
}

impl<'a> Maxp<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        data.range(0, 6)?;
        Some(Self { data })
    }

    pub fn glyph_count(&self) -> u16 {
        self.data.u16_at(4).unwrap_or(0)
    }

    pub fn max_points(&self) -> Option<u16> {
        self.data.u16_at(6)
    }

    pub fn max_contours(&self) -> Option<u16> {
        self.data.u16_at(8)
    }

    pub fn max_composite_points(&self) -> Option<u16> {
        self.data.u16_at(10)
    }

    pub fn max_composite_contours(&self) -> Option<u16> {
        self.data.u16_at(12)
    }

    pub fn max_component_elements(&self) -> Option<u16> {
        self.data.u16_at(28)
    }

    pub fn max_component_depth(&self) -> Option<u16> {
        self.data.u16_at(30)
    }
}

impl<'a> Font<'a> {
    pub fn maxp(&self) -> Option<Maxp<'a>> {
        Maxp::new(self.table_data(TableId::Maxp)?)
    }
}
