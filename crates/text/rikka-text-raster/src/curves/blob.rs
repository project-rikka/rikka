use rikka_text_font::Bounds;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CurveBlob {
    pub(crate) words: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BandAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveHeader {
    pub bounds: Bounds,
    pub curve_count: u16,
    pub horizontal_bands: u8,
    pub vertical_bands: u8,
}

impl CurveBlob {
    pub const COUNTS_WORD: usize = 0;
    pub const BOUNDS_WORD: usize = 1;
    pub const BAND_HEADERS_WORD: usize = 5;
    pub const CURVE_WORDS: usize = 3;
    pub const CURVE_COUNT_SHIFT: u32 = 0;
    pub const HORIZONTAL_BANDS_SHIFT: u32 = 16;
    pub const VERTICAL_BANDS_SHIFT: u32 = 24;
    pub const BAND_OFFSET_MASK: u32 = 0xFFFF;
    pub const BAND_COUNT_SHIFT: u32 = 16;
    pub const REFERENCES_PER_WORD: usize = 2;
    pub const UNORM_MAX: f32 = 65535.0;
    pub const MAX_WORDS: usize = 1 << 16;
    pub const MAX_BANDS: usize = 32;

    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn words(&self) -> &[u32] {
        &self.words
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    pub fn header(&self) -> Option<CurveHeader> {
        CurveHeader::read(&self.words)
    }
}

impl CurveHeader {
    pub fn read(words: &[u32]) -> Option<Self> {
        let counts = *words.get(CurveBlob::COUNTS_WORD)?;
        let bound = |index: usize| -> Option<f32> {
            words
                .get(CurveBlob::BOUNDS_WORD + index)
                .map(|&bits| f32::from_bits(bits))
        };
        let header = Self {
            bounds: Bounds {
                x_min: bound(0)?,
                y_min: bound(1)?,
                x_max: bound(2)?,
                y_max: bound(3)?,
            },
            curve_count: (counts >> CurveBlob::CURVE_COUNT_SHIFT) as u16,
            horizontal_bands: (counts >> CurveBlob::HORIZONTAL_BANDS_SHIFT) as u8,
            vertical_bands: (counts >> CurveBlob::VERTICAL_BANDS_SHIFT) as u8,
        };
        let complete = header.horizontal_bands > 0
            && header.vertical_bands > 0
            && words.len()
                >= CurveBlob::BAND_HEADERS_WORD
                    + usize::from(header.horizontal_bands)
                    + usize::from(header.vertical_bands);
        complete.then_some(header)
    }
}
