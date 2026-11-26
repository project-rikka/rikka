use std::fmt;
use std::ops::RangeInclusive;

use rikka_types::{Choice, Transform};

use crate::catalog::instance_key;
use crate::key::Key;
use crate::schema::Schema;

pub const DOMAIN: &str = "rikka.displays";

pub const INSTANCES: &str = "display";

pub const CONNECTED: &str = "connected";

pub const NAME: &str = "name";
pub const MODES: &str = "modes";
pub const ENABLED: &str = "enabled";
pub const MODE: &str = "mode";
pub const POSITION: &str = "position";
pub const SCALE: &str = "scale";
pub const TRANSFORM: &str = "transform";
pub const ADAPTIVE_SYNC: &str = "adaptive-sync";
pub const HDR: &str = "hdr";
pub const SDR_BRIGHTNESS: &str = "sdr-brightness";

pub const NEVER: &str = "never";
pub const AUTOMATIC: &str = "automatic";
pub const ALWAYS: &str = "always";

pub const SCALES: RangeInclusive<f64> = 0.5..=4.0;
pub const SDR_BRIGHTNESS_RANGE: RangeInclusive<f64> = 80.0..=500.0;
pub const SDR_BRIGHTNESS_DEFAULT: f64 = 203.0;

pub const TRANSFORMS: [&str; 8] = [
    "normal",
    "90",
    "180",
    "270",
    "flipped",
    "flipped-90",
    "flipped-180",
    "flipped-270",
];

const SEPARATOR: char = ';';

#[must_use]
pub fn key(display: &str, suffix: &str) -> String {
    instance_key(display, suffix)
}

#[must_use]
pub fn template() -> Schema {
    Schema::new(DOMAIN)
        .summary("Each display, named by its make, model and serial number")
        .key(Key::bool(ENABLED, true).summary("Whether the display is enabled"))
        .key(Key::string(MODE, "").summary(
            "Resolution and refresh rate as `<width>x<height>@<hertz>`, one of the display's modes",
        ))
        .key(Key::string(POSITION, "0,0").summary("Top-left position in the layout (`x,y`)"))
        .key(Key::float(SCALE, 1.0).summary("Scale factor").range(SCALES))
        .key(
            Key::string(TRANSFORM, TRANSFORMS[0])
                .summary("Rotation and reflection")
                .choices(TRANSFORMS),
        )
        .key(Key::choice::<AdaptiveSync>(ADAPTIVE_SYNC).summary(
            "Variable refresh rate: never, while a fullscreen window shows moving content, or always",
        ))
        .key(
            Key::bool(HDR, false)
                .summary("High dynamic range and wide colour output, on displays capable of it"),
        )
        .key(
            Key::float(SDR_BRIGHTNESS, SDR_BRIGHTNESS_DEFAULT)
                .summary("Brightness of standard content in HDR (cd/m²)")
                .range(SDR_BRIGHTNESS_RANGE),
        )
}

#[must_use]
pub fn listed<T: fmt::Display>(items: impl IntoIterator<Item = T>) -> String {
    let items: Vec<String> = items.into_iter().map(|item| item.to_string()).collect();
    items.join(SEPARATOR.encode_utf8(&mut [0; 4]))
}

pub fn list(text: &str) -> impl Iterator<Item = &str> {
    text.split(SEPARATOR).filter(|item| !item.is_empty())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
}

impl Mode {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (size, refresh) = text.split_once('@')?;
        let (width, height) = size.split_once('x')?;
        let refresh: f64 = refresh.parse().ok()?;
        (refresh.is_finite() && refresh >= 0.0).then_some(())?;
        Some(Self {
            width: width.parse().ok()?,
            height: height.parse().ok()?,
            refresh: (refresh * 1000.0).round() as u32,
        })
    }

    #[must_use]
    pub fn parse_all(text: &str) -> Vec<Self> {
        list(text).filter_map(Self::parse).collect()
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}x{}@{:.3}",
            self.width,
            self.height,
            f64::from(self.refresh) / 1000.0
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AdaptiveSync {
    Never,
    #[default]
    Automatic,
    Always,
}

impl Choice for AdaptiveSync {
    const ALL: &'static [AdaptiveSync] = &[
        AdaptiveSync::Never,
        AdaptiveSync::Automatic,
        AdaptiveSync::Always,
    ];

    fn slug(self) -> &'static str {
        match self {
            AdaptiveSync::Never => NEVER,
            AdaptiveSync::Automatic => AUTOMATIC,
            AdaptiveSync::Always => ALWAYS,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (x, y) = text.split_once(',')?;
        Some(Self {
            x: x.trim().parse().ok()?,
            y: y.trim().parse().ok()?,
        })
    }
}

impl fmt::Display for Position {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{},{}", self.x, self.y)
    }
}

#[must_use]
pub fn transform_name(transform: Transform) -> &'static str {
    TRANSFORMS[transform.to_wayland() as usize]
}

#[must_use]
pub fn transform_named(name: &str) -> Option<Transform> {
    let index = TRANSFORMS.iter().position(|known| *known == name)?;
    Transform::from_wayland(u32::try_from(index).ok()?)
}
