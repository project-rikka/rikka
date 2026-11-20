use crate::hex;
use crate::knob::{Scale, Unit};
use crate::parameters::{Parameters, knobs};
use rikka_i18n::{Label, tr};
use rikka_image_edit::{self as imaging, Operation};
use rikka_types::Color;
use serde::{Deserialize, Serialize};

knobs! {
    for PhotoFilter;
    DENSITY = "density" => density: || tr!("effects.photo_filter.density"), Scale::new(1.0..=100.0).unit(Unit::Percent);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Filter {
    #[default]
    Warming85,
    WarmingLba,
    Warming81,
    Cooling80,
    CoolingLbb,
    Cooling82,
    Red,
    Orange,
    Yellow,
    Green,
    Cyan,
    Blue,
    Violet,
    Magenta,
    Sepia,
    DeepRed,
    DeepBlue,
    DeepEmerald,
    DeepYellow,
    Underwater,
}

impl Filter {
    pub fn color(self) -> Color {
        Color::hex(match self {
            Filter::Warming85 => 0xEC8A00,
            Filter::WarmingLba => 0xFA9600,
            Filter::Warming81 => 0xEBB113,
            Filter::Cooling80 => 0x006DFF,
            Filter::CoolingLbb => 0x005DFF,
            Filter::Cooling82 => 0x00B5FF,
            Filter::Red => 0xEA1A1A,
            Filter::Orange => 0xF38417,
            Filter::Yellow => 0xF9E31C,
            Filter::Green => 0x19C919,
            Filter::Cyan => 0x1DCBEA,
            Filter::Blue => 0x1D35EA,
            Filter::Violet => 0x9B1DEA,
            Filter::Magenta => 0xE31DE3,
            Filter::Sepia => 0xAC7A33,
            Filter::DeepRed => 0xFF0000,
            Filter::DeepBlue => 0x0022CD,
            Filter::DeepEmerald => 0x008C00,
            Filter::DeepYellow => 0xFFD500,
            Filter::Underwater => 0x00C1B1,
        })
    }

    pub fn name(self) -> Label {
        match self {
            Filter::Warming85 => tr!("effects.photo_filter.warming_85"),
            Filter::WarmingLba => tr!("effects.photo_filter.warming_lba"),
            Filter::Warming81 => tr!("effects.photo_filter.warming_81"),
            Filter::Cooling80 => tr!("effects.photo_filter.cooling_80"),
            Filter::CoolingLbb => tr!("effects.photo_filter.cooling_lbb"),
            Filter::Cooling82 => tr!("effects.photo_filter.cooling_82"),
            Filter::Red => tr!("effects.photo_filter.red"),
            Filter::Orange => tr!("effects.photo_filter.orange"),
            Filter::Yellow => tr!("effects.photo_filter.yellow"),
            Filter::Green => tr!("effects.photo_filter.green"),
            Filter::Cyan => tr!("effects.photo_filter.cyan"),
            Filter::Blue => tr!("effects.photo_filter.blue"),
            Filter::Violet => tr!("effects.photo_filter.violet"),
            Filter::Magenta => tr!("effects.photo_filter.magenta"),
            Filter::Sepia => tr!("effects.photo_filter.sepia"),
            Filter::DeepRed => tr!("effects.photo_filter.deep_red"),
            Filter::DeepBlue => tr!("effects.photo_filter.deep_blue"),
            Filter::DeepEmerald => tr!("effects.photo_filter.deep_emerald"),
            Filter::DeepYellow => tr!("effects.photo_filter.deep_yellow"),
            Filter::Underwater => tr!("effects.photo_filter.underwater"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PhotoFilter {
    pub custom: bool,
    pub filter: Filter,
    #[serde(with = "hex")]
    pub color: Color,
    density: f32,
    pub preserve_luminosity: bool,
}

impl Default for PhotoFilter {
    fn default() -> PhotoFilter {
        PhotoFilter {
            custom: false,
            filter: Filter::default(),
            color: Filter::default().color(),
            density: 25.0,
            preserve_luminosity: true,
        }
    }
}

impl PhotoFilter {
    fn chosen(&self) -> Color {
        match self.custom {
            true => self.color,
            false => self.filter.color(),
        }
    }
}

impl Parameters for PhotoFilter {
    fn operation(&self) -> Option<Operation> {
        Some(
            imaging::PhotoFilter::new(self.chosen())
                .density(self.density)
                .preserve_luminosity(self.preserve_luminosity)
                .into(),
        )
    }
}
