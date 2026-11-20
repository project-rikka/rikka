#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Layout {
    L8,
    La8,
    Rgb8,
    Rgba8,
    L16,
    La16,
    Rgb16,
    Rgba16,
}

impl Layout {
    #[must_use]
    pub const fn channels(self) -> usize {
        match self {
            Layout::L8 | Layout::L16 => 1,
            Layout::La8 | Layout::La16 => 2,
            Layout::Rgb8 | Layout::Rgb16 => 3,
            Layout::Rgba8 | Layout::Rgba16 => 4,
        }
    }

    #[must_use]
    pub const fn is_deep(self) -> bool {
        matches!(
            self,
            Layout::L16 | Layout::La16 | Layout::Rgb16 | Layout::Rgba16
        )
    }

    #[must_use]
    pub const fn has_alpha(self) -> bool {
        matches!(
            self,
            Layout::La8 | Layout::Rgba8 | Layout::La16 | Layout::Rgba16
        )
    }

    #[must_use]
    pub const fn bytes_per_pixel(self) -> usize {
        self.channels() * if self.is_deep() { 2 } else { 1 }
    }

    pub(crate) const fn new(channels: usize, deep: bool) -> Layout {
        match (channels, deep) {
            (1, false) => Layout::L8,
            (2, false) => Layout::La8,
            (3, false) => Layout::Rgb8,
            (1, true) => Layout::L16,
            (2, true) => Layout::La16,
            (3, true) => Layout::Rgb16,
            (_, true) => Layout::Rgba16,
            (_, false) => Layout::Rgba8,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_width: u32,
    pub max_height: u32,
    pub max_bytes: u64,
}

impl Limits {
    pub const UNLIMITED: Limits = Limits {
        max_width: u32::MAX,
        max_height: u32::MAX,
        max_bytes: u64::MAX,
    };
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Samples {
    U8(Vec<u8>),
    U16(Vec<u16>),
}

impl Samples {
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Samples::U8(samples) => samples,
            Samples::U16(samples) => bytemuck::cast_slice(samples),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub layout: Layout,
    pub samples: Samples,
}
