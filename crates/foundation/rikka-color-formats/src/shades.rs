use crate::rgb::Rgb;
use crate::spaces::Hsv;

const BRIGHT: f64 = 0.15;
const DIM: f64 = 0.3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shades {
    pub lightest: Rgb,
    pub lighter: Rgb,
    pub darker: Rgb,
    pub darkest: Rgb,
}

impl Shades {
    pub(crate) fn new(rgb: Rgb) -> Shades {
        let Hsv {
            hue,
            saturation,
            value,
        } = rgb.hsv();
        let warm = f64::from(u8::from(1.0 - value < BRIGHT));
        let cool = f64::from(u8::from(value - DIM < 0.0));
        let shade = |hue: f64, value: f64| {
            Hsv {
                hue,
                saturation,
                value,
            }
            .rgb()
        };
        Shades {
            lightest: shade((hue + warm * 8.0).min(360.0), (value + 0.3).min(1.0)),
            lighter: shade((hue + warm * 4.0).min(360.0), (value + 0.15).min(1.0)),
            darker: shade((hue - cool * 4.0).max(0.0), (value - 0.2).max(0.0)),
            darkest: shade((hue - cool * 8.0).max(0.0), (value - 0.3).max(0.0)),
        }
    }

    #[must_use]
    pub fn all(self) -> [Rgb; 4] {
        [self.lightest, self.lighter, self.darker, self.darkest]
    }
}
