#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Level {
    Store,
    Fastest,
    Fast,
    #[default]
    Balanced,
    Best,
    Precise(u8),
}

impl Level {
    #[must_use]
    pub const fn scale(self) -> u8 {
        match self {
            Level::Store => 0,
            Level::Fastest => 1,
            Level::Fast => 3,
            Level::Balanced => 6,
            Level::Best => 9,
            Level::Precise(value) => {
                if value > 9 {
                    9
                } else {
                    value
                }
            }
        }
    }

    #[must_use]
    pub const fn is_store(self) -> bool {
        self.scale() == 0
    }

    pub(crate) fn deflate(self) -> flate2::Compression {
        flate2::Compression::new(u32::from(self.scale()))
    }

    pub(crate) fn bzip2(self) -> bzip2::Compression {
        bzip2::Compression::new(u32::from(self.scale()).max(1))
    }

    pub(crate) fn preset(self) -> u32 {
        u32::from(self.scale())
    }

    pub(crate) fn zstd(self) -> i32 {
        const ZSTD_LEVELS: [i32; 10] = [1, 1, 3, 5, 7, 9, 11, 13, 16, 19];
        ZSTD_LEVELS[usize::from(self.scale())]
    }
}
