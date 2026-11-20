#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Self = Self::rgba(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);

    #[inline]
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    #[inline]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::rgba(r, g, b, 1.0)
    }

    #[inline]
    pub fn rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::rgba(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        )
    }

    #[inline]
    pub fn rgb8(r: u8, g: u8, b: u8) -> Self {
        Self::rgba8(r, g, b, 255)
    }

    #[inline]
    pub fn hex(rgb: u32) -> Self {
        Self::rgb8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
    }

    pub fn parse_hex(text: &str) -> Option<Self> {
        let hex = text.strip_prefix('#')?;
        let mut channels = [0u8, 0, 0, 255];
        match hex.len() {
            3 | 4 => {
                for (channel, digit) in channels.iter_mut().zip(hex.chars()) {
                    *channel = digit.to_digit(16)? as u8 * 0x11;
                }
            }
            6 | 8 => {
                for (channel, pair) in channels.iter_mut().zip(hex.as_bytes().chunks(2)) {
                    *channel = u8::from_str_radix(core::str::from_utf8(pair).ok()?, 16).ok()?;
                }
            }
            _ => return None,
        }
        let [r, g, b, a] = channels;
        Some(Self::rgba8(r, g, b, a))
    }

    #[must_use]
    pub fn to_rgba8(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a].map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8)
    }

    #[must_use]
    pub fn to_hex(self) -> String {
        let [r, g, b, a] = self.to_rgba8();
        match a {
            255 => format!("#{r:02x}{g:02x}{b:02x}"),
            _ => format!("#{r:02x}{g:02x}{b:02x}{a:02x}"),
        }
    }

    #[must_use]
    pub fn from_hsv(hue: f32, saturation: f32, value: f32) -> Self {
        let sector = hue.rem_euclid(1.0) * 6.0;
        let chroma = value * saturation;
        let rising = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
        let (r, g, b) = match sector as u32 {
            0 => (chroma, rising, 0.0),
            1 => (rising, chroma, 0.0),
            2 => (0.0, chroma, rising),
            3 => (0.0, rising, chroma),
            4 => (rising, 0.0, chroma),
            _ => (chroma, 0.0, rising),
        };
        let floor = value - chroma;
        Self::rgb(r + floor, g + floor, b + floor)
    }

    #[must_use]
    pub fn to_hsv(self) -> [f32; 3] {
        let (r, g, b) = (
            self.r.clamp(0.0, 1.0),
            self.g.clamp(0.0, 1.0),
            self.b.clamp(0.0, 1.0),
        );
        let value = r.max(g).max(b);
        let chroma = value - r.min(g).min(b);
        let saturation = match value > 0.0 {
            true => chroma / value,
            false => 0.0,
        };
        let sector = match chroma > 0.0 {
            false => 0.0,
            true if value == r => ((g - b) / chroma).rem_euclid(6.0),
            true if value == g => (b - r) / chroma + 2.0,
            true => (r - g) / chroma + 4.0,
        };
        [sector / 6.0, saturation, value]
    }

    #[inline]
    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    #[inline]
    pub fn scale_alpha(self, factor: f32) -> Self {
        self.with_alpha(self.a * factor.clamp(0.0, 1.0))
    }

    #[inline]
    pub fn mix(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self::rgba(
            self.r + (other.r - self.r) * t,
            self.g + (other.g - self.g) * t,
            self.b + (other.b - self.b) * t,
            self.a,
        )
    }

    #[inline]
    pub fn to_premultiplied(self) -> [f32; 4] {
        [self.r * self.a, self.g * self.a, self.b * self.a, self.a]
    }

    #[inline]
    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    #[inline]
    pub fn is_transparent(self) -> bool {
        self.a <= 0.0
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::TRANSPARENT
    }
}
