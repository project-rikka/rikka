use crate::outline::bounds::Bounds;
use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::table_id::TableId;

const MAC_STYLE_BOLD: u16 = 0x0001;
const MAC_STYLE_ITALIC: u16 = 0x0002;

#[derive(Clone, Copy, Debug)]
pub struct Head<'a> {
    data: &'a [u8],
}

impl<'a> Head<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        data.range(0, 54)?;
        Some(Self { data })
    }

    #[inline]
    fn u16(&self, offset: usize) -> u16 {
        self.data.u16_at(offset).unwrap_or(0)
    }

    #[inline]
    fn i16(&self, offset: usize) -> i16 {
        self.data.i16_at(offset).unwrap_or(0)
    }

    pub fn font_revision(&self) -> f32 {
        self.data.fixed_at(4).unwrap_or(0.0)
    }

    pub fn flags(&self) -> u16 {
        self.u16(16)
    }

    pub fn units_per_em(&self) -> u16 {
        self.u16(18)
    }

    pub fn bounds(&self) -> Bounds {
        Bounds {
            x_min: f32::from(self.i16(36)),
            y_min: f32::from(self.i16(38)),
            x_max: f32::from(self.i16(40)),
            y_max: f32::from(self.i16(42)),
        }
    }

    pub fn mac_style(&self) -> u16 {
        self.u16(44)
    }

    pub fn is_bold(&self) -> bool {
        self.mac_style() & MAC_STYLE_BOLD != 0
    }

    pub fn is_italic(&self) -> bool {
        self.mac_style() & MAC_STYLE_ITALIC != 0
    }

    pub fn lowest_recommended_ppem(&self) -> u16 {
        self.u16(46)
    }

    pub fn index_to_loc_format(&self) -> i16 {
        self.i16(50)
    }
}

impl<'a> Font<'a> {
    pub fn head(&self) -> Option<Head<'a>> {
        Head::new(self.table_data(TableId::Head)?)
    }
}
