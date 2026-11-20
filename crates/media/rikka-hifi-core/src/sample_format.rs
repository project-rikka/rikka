#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleFormat {
    S16,
    S24Packed,
    S24,
    S32,
    F32,
    F64,
    Dsd8,
    Dsd16Le,
    Dsd16Be,
    Dsd32Le,
    Dsd32Be,
}

impl SampleFormat {
    pub const INTEGER: [SampleFormat; 4] = [
        SampleFormat::S32,
        SampleFormat::S24,
        SampleFormat::S24Packed,
        SampleFormat::S16,
    ];

    pub const DSD: [SampleFormat; 5] = [
        SampleFormat::Dsd32Be,
        SampleFormat::Dsd32Le,
        SampleFormat::Dsd16Be,
        SampleFormat::Dsd16Le,
        SampleFormat::Dsd8,
    ];

    #[must_use]
    pub fn bytes(self) -> usize {
        match self {
            SampleFormat::Dsd8 => 1,
            SampleFormat::S16 | SampleFormat::Dsd16Le | SampleFormat::Dsd16Be => 2,
            SampleFormat::S24Packed => 3,
            SampleFormat::S24
            | SampleFormat::S32
            | SampleFormat::F32
            | SampleFormat::Dsd32Le
            | SampleFormat::Dsd32Be => 4,
            SampleFormat::F64 => 8,
        }
    }

    #[must_use]
    pub fn precision(self) -> u32 {
        match self {
            SampleFormat::S16 => 16,
            SampleFormat::S24Packed | SampleFormat::S24 | SampleFormat::F32 => 24,
            SampleFormat::S32 => 32,
            SampleFormat::F64 => 53,
            SampleFormat::Dsd8
            | SampleFormat::Dsd16Le
            | SampleFormat::Dsd16Be
            | SampleFormat::Dsd32Le
            | SampleFormat::Dsd32Be => 1,
        }
    }

    #[must_use]
    pub fn is_integer(self) -> bool {
        Self::INTEGER.contains(&self)
    }

    #[must_use]
    pub fn is_float(self) -> bool {
        matches!(self, SampleFormat::F32 | SampleFormat::F64)
    }

    #[must_use]
    pub fn is_dsd(self) -> bool {
        Self::DSD.contains(&self)
    }

    #[must_use]
    pub fn carries_dop(self) -> bool {
        matches!(
            self,
            SampleFormat::S32 | SampleFormat::S24 | SampleFormat::S24Packed
        )
    }
}

impl std::fmt::Display for SampleFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SampleFormat::S16 => "S16_LE",
            SampleFormat::S24Packed => "S24_3LE",
            SampleFormat::S24 => "S24_LE",
            SampleFormat::S32 => "S32_LE",
            SampleFormat::F32 => "F32_LE",
            SampleFormat::F64 => "F64_LE",
            SampleFormat::Dsd8 => "DSD_U8",
            SampleFormat::Dsd16Le => "DSD_U16_LE",
            SampleFormat::Dsd16Be => "DSD_U16_BE",
            SampleFormat::Dsd32Le => "DSD_U32_LE",
            SampleFormat::Dsd32Be => "DSD_U32_BE",
        })
    }
}
