use cut_project::{Format, Rate};

const MAX_SCALE: f64 = 4.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Resolution {
    #[default]
    Project,
    Short(u32),
}

impl Resolution {
    pub const SHORT_SIDES: [u32; 6] = [2160, 1440, 1080, 720, 480, 360];

    pub fn offered(format: Format) -> impl Iterator<Item = Resolution> {
        std::iter::once(Resolution::Project).chain(
            Resolution::SHORT_SIDES
                .into_iter()
                .map(Resolution::Short)
                .filter(move |resolution| resolution.fits(format)),
        )
    }

    #[must_use]
    pub fn fits(self, format: Format) -> bool {
        match self {
            Resolution::Project => true,
            Resolution::Short(side) => {
                let short = format.width.min(format.height).max(1);
                f64::from(side) / f64::from(short) <= MAX_SCALE
            }
        }
    }

    #[must_use]
    pub fn size(self, format: Format) -> (u32, u32) {
        let (width, height) = (
            f64::from(format.width.max(1)),
            f64::from(format.height.max(1)),
        );
        let scale = match self {
            Resolution::Project => 1.0,
            Resolution::Short(side) => (f64::from(side) / width.min(height)).min(MAX_SCALE),
        };
        (even(width * scale), even(height * scale))
    }
}

fn even(length: f64) -> u32 {
    ((length / 2.0).round() as u32 * 2).max(2)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FrameRate {
    #[default]
    Project,
    Fixed(Rate),
}

impl FrameRate {
    pub const FIXED: [Rate; 8] = Rate::COMMON;

    pub fn offered() -> impl Iterator<Item = FrameRate> {
        std::iter::once(FrameRate::Project).chain(FrameRate::FIXED.map(FrameRate::Fixed))
    }

    #[must_use]
    pub fn rate(self, project: Rate) -> Rate {
        match self {
            FrameRate::Project => project,
            FrameRate::Fixed(rate) => rate,
        }
    }
}
