use rikka_types::Rect;

pub const SMALLEST_SIDE: u32 = 16;
pub const LARGEST_SIDE: u32 = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Resolution {
    width: u32,
    height: u32,
}

impl Resolution {
    pub const FULL_HD: Self = Self {
        width: 1920,
        height: 1080,
    };

    pub fn new(width: u32, height: u32) -> Option<Self> {
        (fits(width) && fits(height)).then_some(Self { width, height })
    }

    pub fn width(self) -> u32 {
        self.width
    }

    pub fn height(self) -> u32 {
        self.height
    }

    pub fn is_portrait(self) -> bool {
        self.height > self.width
    }

    pub fn is_even(self) -> bool {
        self.width.is_multiple_of(2) && self.height.is_multiple_of(2)
    }

    pub fn is_square(self) -> bool {
        self.height == self.width
    }

    pub fn turned(self) -> Self {
        Self {
            width: self.height,
            height: self.width,
        }
    }

    pub fn frame_in(self, bounds: Rect, scale: f32) -> Rect {
        let left = (bounds.min_x() * scale).round();
        let top = (bounds.min_y() * scale).round();
        let across = (bounds.max_x() * scale).round() - left;
        let down = (bounds.max_y() * scale).round() - top;
        if across < 1.0 || down < 1.0 {
            return Rect::new(bounds.x(), bounds.y(), 0.0, 0.0);
        }
        let aspect = f64::from(self.width) / f64::from(self.height);
        let (across, down) = (f64::from(across), f64::from(down));
        let (width, height) = match across / down > aspect {
            true => ((down * aspect).round().max(1.0), down),
            false => (across, (across / aspect).round().max(1.0)),
        };
        let x = f64::from(left) + ((across - width) / 2.0).floor();
        let y = f64::from(top) + ((down - height) / 2.0).floor();
        let scale = f64::from(scale);
        Rect::new(
            (x / scale) as f32,
            (y / scale) as f32,
            (width / scale) as f32,
            (height / scale) as f32,
        )
    }
}

impl Default for Resolution {
    fn default() -> Self {
        Self::FULL_HD
    }
}

pub fn parse_side(typed: &str) -> Option<u32> {
    typed.trim().parse::<u32>().ok().filter(|side| fits(*side))
}

fn fits(side: u32) -> bool {
    (SMALLEST_SIDE..=LARGEST_SIDE).contains(&side)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolutionPreset {
    landscape: Resolution,
    ratio: (u32, u32),
}

pub const RESOLUTION_PRESETS: [ResolutionPreset; 7] = [
    ResolutionPreset::new(3840, 2160, (16, 9)),
    ResolutionPreset::new(2560, 1440, (16, 9)),
    ResolutionPreset::new(1920, 1080, (16, 9)),
    ResolutionPreset::new(1280, 720, (16, 9)),
    ResolutionPreset::new(2560, 1080, (21, 9)),
    ResolutionPreset::new(1440, 1080, (4, 3)),
    ResolutionPreset::new(1080, 1080, (1, 1)),
];

impl ResolutionPreset {
    const fn new(width: u32, height: u32, ratio: (u32, u32)) -> Self {
        Self {
            landscape: Resolution { width, height },
            ratio,
        }
    }

    pub fn facing(self, like: Resolution) -> Resolution {
        match like.is_portrait() {
            true => self.landscape.turned(),
            false => self.landscape,
        }
    }

    pub fn ratio_facing(self, like: Resolution) -> (u32, u32) {
        let (across, down) = self.ratio;
        match like.is_portrait() {
            true => (down, across),
            false => (across, down),
        }
    }
}

pub fn resolution_preset(resolution: Resolution) -> Option<usize> {
    RESOLUTION_PRESETS
        .iter()
        .position(|preset| preset.facing(resolution) == resolution)
}
