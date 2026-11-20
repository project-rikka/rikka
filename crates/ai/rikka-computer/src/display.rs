#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Display {
    width: u32,
    height: u32,
}

const MAX_WIDTH: u32 = 3840;
const MAX_HEIGHT: u32 = 2160;

const MIN_WIDTH: u32 = 320;
const MIN_HEIGHT: u32 = 240;

impl Default for Display {
    fn default() -> Display {
        Display {
            width: 1280,
            height: 800,
        }
    }
}

impl Display {
    #[must_use]
    pub fn new(width: u32, height: u32) -> Display {
        Display {
            width: width.clamp(MIN_WIDTH, MAX_WIDTH),
            height: height.clamp(MIN_HEIGHT, MAX_HEIGHT),
        }
    }

    #[must_use]
    pub fn width(self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn capture_size(self) -> (u32, u32) {
        let limit = Display::default();
        if self.width <= limit.width && self.height <= limit.height {
            return (self.width, self.height);
        }

        let scale = f64::from(limit.width) / f64::from(self.width);
        let scale = scale.min(f64::from(limit.height) / f64::from(self.height));
        (
            scaled(self.width, scale).max(1),
            scaled(self.height, scale).max(1),
        )
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "downscales a u32 length by a factor below one"
)]
fn scaled(length: u32, scale: f64) -> u32 {
    (f64::from(length) * scale).round() as u32
}

impl std::fmt::Display for Display {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}x{}", self.width, self.height)
    }
}
