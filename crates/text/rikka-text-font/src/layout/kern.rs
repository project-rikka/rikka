use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;
use crate::sfnt::table_id::TableId;

const OT_HORIZONTAL: u16 = 0x0001;
const OT_MINIMUM: u16 = 0x0002;
const OT_CROSS_STREAM: u16 = 0x0004;
const OT_OVERRIDE: u16 = 0x0008;
const APPLE_VERTICAL: u16 = 0x8000;
const APPLE_CROSS_STREAM: u16 = 0x4000;
const APPLE_VARIATION: u16 = 0x2000;

#[derive(Clone, Copy, Debug)]
pub struct Kern<'a> {
    subtables: &'a [u8],
    count: u32,
    apple: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct KernSubtable<'a> {
    data: &'a [u8],
    body: usize,
    format: u8,
    horizontal: bool,
    cross_stream: bool,
    minimum: bool,
    override_sum: bool,
    variation: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct KernSubtables<'a> {
    rest: &'a [u8],
    remaining: u32,
    apple: bool,
}

impl<'a> Kern<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        match data.u16_at(0)? {
            0 => Some(Self {
                count: u32::from(data.u16_at(2)?),
                subtables: data.tail(4)?,
                apple: false,
            }),
            1 if data.u16_at(2)? == 0 => Some(Self {
                count: data.u32_at(4)?,
                subtables: data.tail(8)?,
                apple: true,
            }),
            _ => None,
        }
    }

    #[inline]
    pub fn is_apple(&self) -> bool {
        self.apple
    }

    pub fn subtables(&self) -> KernSubtables<'a> {
        KernSubtables {
            rest: self.subtables,
            remaining: self.count,
            apple: self.apple,
        }
    }

    pub fn horizontal_kerning(&self, left: GlyphId, right: GlyphId) -> Option<i32> {
        let mut total = None;
        for subtable in self.subtables() {
            if !subtable.horizontal
                || subtable.cross_stream
                || subtable.variation
                || subtable.minimum
            {
                continue;
            }
            let Some(value) = subtable.kerning(left, right) else {
                continue;
            };
            total = Some(if subtable.override_sum {
                value
            } else {
                total.unwrap_or(0) + value
            });
        }
        total
    }
}

impl<'a> Iterator for KernSubtables<'a> {
    type Item = KernSubtable<'a>;

    fn next(&mut self) -> Option<KernSubtable<'a>> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        let parsed = if self.apple {
            self.next_apple()
        } else {
            self.next_opentype()
        };
        if parsed.is_none() {
            self.remaining = 0;
        }
        parsed
    }
}

impl<'a> KernSubtables<'a> {
    fn next_opentype(&mut self) -> Option<KernSubtable<'a>> {
        let rest = self.rest;
        let declared = usize::from(rest.u16_at(2)?);
        let coverage = rest.u16_at(4)?;
        let format = (coverage >> 8) as u8;
        let length = if self.remaining == 0 && format == 0 {
            rest.len()
        } else {
            declared.max(6).min(rest.len())
        };
        let data = rest.range(0, length)?;
        self.rest = rest.tail(length)?;
        Some(KernSubtable {
            data,
            body: 6,
            format,
            horizontal: coverage & OT_HORIZONTAL != 0,
            cross_stream: coverage & OT_CROSS_STREAM != 0,
            minimum: coverage & OT_MINIMUM != 0,
            override_sum: coverage & OT_OVERRIDE != 0,
            variation: false,
        })
    }

    fn next_apple(&mut self) -> Option<KernSubtable<'a>> {
        let rest = self.rest;
        let length = (rest.u32_at(0)? as usize).max(8).min(rest.len());
        let coverage = rest.u16_at(4)?;
        let data = rest.range(0, length)?;
        self.rest = rest.tail(length)?;
        Some(KernSubtable {
            data,
            body: 8,
            format: coverage as u8,
            horizontal: coverage & APPLE_VERTICAL == 0,
            cross_stream: coverage & APPLE_CROSS_STREAM != 0,
            minimum: false,
            override_sum: false,
            variation: coverage & APPLE_VARIATION != 0,
        })
    }
}

impl KernSubtable<'_> {
    #[inline]
    pub fn format(&self) -> u8 {
        self.format
    }

    #[inline]
    pub fn is_horizontal(&self) -> bool {
        self.horizontal
    }

    #[inline]
    pub fn is_cross_stream(&self) -> bool {
        self.cross_stream
    }

    #[inline]
    pub fn is_minimum(&self) -> bool {
        self.minimum
    }

    #[inline]
    pub fn is_override(&self) -> bool {
        self.override_sum
    }

    #[inline]
    pub fn is_variation(&self) -> bool {
        self.variation
    }

    pub fn kerning(&self, left: GlyphId, right: GlyphId) -> Option<i32> {
        match self.format {
            0 => self.pair_kerning(left, right),
            2 => self.class_kerning(left, right),
            _ => None,
        }
    }

    fn pair_kerning(&self, left: GlyphId, right: GlyphId) -> Option<i32> {
        let body = self.data.tail(self.body)?;
        let count = usize::from(body.u16_at(0)?);
        let pairs = body.tail(8)?;
        let count = count.min(pairs.len() / 6);
        let key = (u32::from(left.0) << 16) | u32::from(right.0);
        let mut low = 0;
        let mut high = count;
        while low < high {
            let middle = low + (high - low) / 2;
            match pairs.u32_at(middle * 6)?.cmp(&key) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return pairs.i16_at(middle * 6 + 4).map(i32::from),
            }
        }
        None
    }

    fn class_kerning(&self, left: GlyphId, right: GlyphId) -> Option<i32> {
        let body = self.body;
        let left_table = usize::from(self.data.u16_at(body + 2)?);
        let right_table = usize::from(self.data.u16_at(body + 4)?);
        let array = usize::from(self.data.u16_at(body + 6)?);
        let left_class = self
            .data
            .tail(left_table)
            .and_then(|table| class_of(table, left))
            .unwrap_or(0);
        let right_class = self
            .data
            .tail(right_table)
            .and_then(|table| class_of(table, right))
            .unwrap_or(0);
        let offset = usize::from(left_class) + usize::from(right_class);
        if offset < array {
            return None;
        }
        self.data.i16_at(offset).map(i32::from)
    }
}

fn class_of(table: &[u8], glyph: GlyphId) -> Option<u16> {
    let first = table.u16_at(0)?;
    let count = table.u16_at(2)?;
    let index = glyph.0.checked_sub(first)?;
    if index >= count {
        return None;
    }
    table.u16_at(4 + usize::from(index) * 2)
}

impl<'a> Font<'a> {
    pub fn kern(&self) -> Option<Kern<'a>> {
        Kern::new(self.table_data(TableId::Kern)?)
    }
}
