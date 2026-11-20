#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat {
    S16,
    S24,
    S24In32,
    S32,
    F32,
    F64,
}

impl SampleFormat {
    pub const ALL: [Self; 6] = [
        Self::S16,
        Self::S24,
        Self::S24In32,
        Self::S32,
        Self::F32,
        Self::F64,
    ];

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|format| format.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::S16 => "S16LE",
            Self::S24 => "S24LE",
            Self::S24In32 => "S24_32LE",
            Self::S32 => "S32LE",
            Self::F32 => "F32LE",
            Self::F64 => "F64LE",
        }
    }

    pub fn width(self) -> u32 {
        match self {
            Self::S16 => 16,
            Self::S24 => 24,
            Self::S24In32 | Self::S32 | Self::F32 => 32,
            Self::F64 => 64,
        }
    }

    pub fn depth(self) -> u32 {
        match self {
            Self::S24In32 => 24,
            format => format.width(),
        }
    }

    pub fn floating(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Format {
    pub sample: SampleFormat,
    pub rate: u32,
    pub channels: u32,
}

impl Format {
    pub fn bitrate(self) -> u64 {
        u64::from(self.sample.width()) * u64::from(self.rate) * u64::from(self.channels)
    }
}

pub const QUANTUMS: [u32; 10] = [32, 64, 128, 256, 512, 1024, 2048, 4096, 6144, 8192];

pub(crate) fn latency(quantum: u32, rate: u32) -> Option<f64> {
    (rate > 0).then(|| f64::from(quantum) * 1000.0 / f64::from(rate))
}
