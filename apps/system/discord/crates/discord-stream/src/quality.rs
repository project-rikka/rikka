const BITS_PER_PIXEL: f64 = 0.06;
const MIN_BITRATE: u32 = 1_000_000;
const MAX_BITRATE: u32 = 40_000_000;
const LARGEST: (u32, u32) = (3840, 2160);
const SMALLEST: u32 = 16;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Resolution {
    Hd,
    #[default]
    FullHd,
    QuadHd,
    UltraHd,
    Source,
}

impl Resolution {
    pub const ALL: [Resolution; 5] = [
        Resolution::Hd,
        Resolution::FullHd,
        Resolution::QuadHd,
        Resolution::UltraHd,
        Resolution::Source,
    ];

    #[must_use]
    pub const fn height(self) -> Option<u32> {
        match self {
            Resolution::Hd => Some(720),
            Resolution::FullHd => Some(1080),
            Resolution::QuadHd => Some(1440),
            Resolution::UltraHd => Some(2160),
            Resolution::Source => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FrameRate {
    Fifteen,
    #[default]
    Thirty,
    Sixty,
}

impl FrameRate {
    pub const ALL: [FrameRate; 3] = [FrameRate::Fifteen, FrameRate::Thirty, FrameRate::Sixty];

    #[must_use]
    pub const fn per_second(self) -> u32 {
        match self {
            FrameRate::Fifteen => 15,
            FrameRate::Thirty => 30,
            FrameRate::Sixty => 60,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Quality {
    pub resolution: Resolution,
    pub frame_rate: FrameRate,
}

impl Quality {
    #[must_use]
    pub fn fit(self, (width, height): (u32, u32)) -> (u32, u32) {
        let width = width.max(SMALLEST);
        let height = height.max(SMALLEST);
        let tallest = self.resolution.height().unwrap_or(LARGEST.1).min(LARGEST.1);
        let scale = (f64::from(tallest) / f64::from(height))
            .min(f64::from(LARGEST.0) / f64::from(width))
            .min(1.0);
        let side = |length: u32| {
            let scaled = (f64::from(length) * scale).round() as u32;
            (scaled & !1).max(SMALLEST)
        };
        (side(width), side(height))
    }

    #[must_use]
    pub fn bitrate(self, (width, height): (u32, u32)) -> u32 {
        let pixels = f64::from(width) * f64::from(height);
        let bits = pixels * f64::from(self.frame_rate.per_second()) * BITS_PER_PIXEL;
        (bits as u32).clamp(MIN_BITRATE, MAX_BITRATE)
    }
}
