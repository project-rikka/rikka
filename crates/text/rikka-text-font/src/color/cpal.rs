use crate::color::color_value::Color;
use crate::parse::array::Array;
use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::table_id::TableId;

const USABLE_WITH_LIGHT_BACKGROUND: u32 = 0x0001;
const USABLE_WITH_DARK_BACKGROUND: u32 = 0x0002;
const NO_NAME: u16 = 0xFFFF;
const COLOR_RECORD_SIZE: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PaletteBackground {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug)]
pub struct Cpal<'a> {
    entry_count: u16,
    colors: &'a [u8],
    first_indices: Array<'a, u16>,
    types: Option<Array<'a, u32>>,
    labels: Option<Array<'a, u16>>,
    entry_labels: Option<Array<'a, u16>>,
}

#[derive(Clone, Copy, Debug)]
pub struct Palette<'a> {
    index: u16,
    colors: &'a [u8],
    flags: u32,
    label: Option<u16>,
}

impl<'a> Cpal<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        let version = data.u16_at(0)?;
        let entry_count = data.u16_at(2)?;
        let palette_count = usize::from(data.u16_at(4)?);
        let record_count = usize::from(data.u16_at(6)?);
        let colors = data.range(data.u32_at(8)? as usize, record_count * COLOR_RECORD_SIZE)?;
        let first_indices = data.array::<u16>(12, palette_count)?;
        let (mut types, mut labels, mut entry_labels) = (None, None, None);
        if version >= 1 {
            let header = 12 + palette_count * 2;
            types = data
                .offset32(0, header)
                .and_then(|table| table.array::<u32>(0, palette_count));
            labels = data
                .offset32(0, header + 4)
                .and_then(|table| table.array::<u16>(0, palette_count));
            entry_labels = data
                .offset32(0, header + 8)
                .and_then(|table| table.array::<u16>(0, usize::from(entry_count)));
        }
        Some(Self {
            entry_count,
            colors,
            first_indices,
            types,
            labels,
            entry_labels,
        })
    }

    #[inline]
    pub fn palette_count(&self) -> u16 {
        self.first_indices.len() as u16
    }

    #[inline]
    pub fn entry_count(&self) -> u16 {
        self.entry_count
    }

    pub fn palette(&self, index: u16) -> Option<Palette<'a>> {
        let first = usize::from(self.first_indices.get(usize::from(index))?);
        let colors = self.colors.range(
            first * COLOR_RECORD_SIZE,
            usize::from(self.entry_count) * COLOR_RECORD_SIZE,
        )?;
        Some(Palette {
            index,
            colors,
            flags: self
                .types
                .and_then(|types| types.get(usize::from(index)))
                .unwrap_or(0),
            label: self
                .labels
                .and_then(|labels| labels.get(usize::from(index)))
                .filter(|&name| name != NO_NAME),
        })
    }

    pub fn palettes(&self) -> impl Iterator<Item = Palette<'a>> + use<'a> {
        let cpal = *self;
        (0..self.palette_count()).filter_map(move |index| cpal.palette(index))
    }

    pub fn palette_for(&self, background: PaletteBackground) -> Option<Palette<'a>> {
        self.palettes()
            .find(|palette| palette.usable_with(background))
            .or_else(|| self.palette(0))
    }

    pub fn entry_label(&self, entry: u16) -> Option<u16> {
        self.entry_labels?
            .get(usize::from(entry))
            .filter(|&name| name != NO_NAME)
    }
}

impl<'a> Palette<'a> {
    #[inline]
    pub fn index(&self) -> u16 {
        self.index
    }

    #[inline]
    pub fn len(&self) -> u16 {
        (self.colors.len() / COLOR_RECORD_SIZE) as u16
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }

    #[inline]
    pub fn color(&self, entry: u16) -> Option<Color> {
        let [b, g, r, a] = self
            .colors
            .chunk::<4>(usize::from(entry) * COLOR_RECORD_SIZE)?;
        Some(Color { r, g, b, a })
    }

    pub fn colors(&self) -> impl ExactSizeIterator<Item = Color> + use<'a> {
        let palette = *self;
        (0..self.len()).map(move |entry| palette.color(entry).unwrap_or_default())
    }

    #[inline]
    pub fn usable_with(&self, background: PaletteBackground) -> bool {
        let flag = match background {
            PaletteBackground::Light => USABLE_WITH_LIGHT_BACKGROUND,
            PaletteBackground::Dark => USABLE_WITH_DARK_BACKGROUND,
        };
        self.flags & flag != 0
    }

    #[inline]
    pub fn label(&self) -> Option<u16> {
        self.label
    }
}

impl<'a> Font<'a> {
    pub fn cpal(&self) -> Option<Cpal<'a>> {
        Cpal::new(self.table_data(TableId::Cpal)?)
    }
}
