use std::f32::consts::{PI, TAU};

use serde::{Deserialize, Serialize};

use crate::asset::PieceKind;
use crate::style::{Curve, Flutter, Inertia, Style};

const HANGING_EXTENT: f32 = 0.85;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Preset {
    Skirt,
    Cape,
    Sleeve,
    Ribbon,
    Accessory,
    LongHair,
    ShortHair,
    Bangs,
    Tail,
}

impl Preset {
    pub const ALL: [Preset; 9] = [
        Preset::Skirt,
        Preset::Cape,
        Preset::Sleeve,
        Preset::Ribbon,
        Preset::Accessory,
        Preset::LongHair,
        Preset::ShortHair,
        Preset::Bangs,
        Preset::Tail,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Preset::Skirt => "skirt",
            Preset::Cape => "cape",
            Preset::Sleeve => "sleeve",
            Preset::Ribbon => "ribbon",
            Preset::Accessory => "accessory",
            Preset::LongHair => "long hair",
            Preset::ShortHair => "short hair",
            Preset::Bangs => "bangs",
            Preset::Tail => "tail",
        }
    }

    #[must_use]
    pub fn kind(self) -> PieceKind {
        match self {
            Preset::LongHair | Preset::ShortHair | Preset::Bangs | Preset::Tail => {
                PieceKind::Strands
            }
            Preset::Skirt | Preset::Cape | Preset::Sleeve | Preset::Ribbon | Preset::Accessory => {
                PieceKind::Sheet
            }
        }
    }

    #[must_use]
    pub fn style(self) -> Style {
        let cloth = cloth();
        let hair = hair();
        match self {
            Preset::Skirt => skirt(&cloth),
            Preset::Cape => Style {
                gravity: 0.7,
                drag: 0.70,
                lift: 0.35,
                compression: 0.24,
                bend: 0.28,
                buckling: 0.35,
                restore: Curve::from_points([[0.0, 0.02], [0.4, 0.005], [1.0, 0.001]]),
                limit: Curve::linear(0.15, 1.05),
                limit_stiffness: 0.45,
                tether: 1.02,
                extent: HANGING_EXTENT,
                attenuation: 0.35,
                flutter: Flutter {
                    amplitude: 0.003,
                    ..cloth.flutter.clone()
                },
                ..cloth
            },
            Preset::Sleeve => Style {
                gravity: 0.4,
                limit: Curve::linear(0.17, 0.79),
                max_distance: Curve::linear(0.4, 1.0),
                backstop: Some(0.01),
                ..cloth
            },
            Preset::Ribbon => Style {
                drag: 0.75,
                lift: 0.30,
                compression: 0.16,
                bend: 0.20,
                buckling: 0.45,
                restore: Curve::linear(0.008, 0.001),
                limit: Curve::linear(1.0, 1.4),
                limit_stiffness: 0.5,
                pull: Curve::flat(0.0),
                backstop: None,
                attenuation: 0.30,
                thickness: 0.003,
                self_collision: false,
                flutter: Flutter {
                    amplitude: 0.003,
                    ..cloth.flutter.clone()
                },
                ..cloth
            },
            Preset::Accessory => Style {
                gravity: 0.6,
                damping: Curve::flat(0.15),
                restore: Curve::linear(0.06, 0.025),
                limit: Curve::linear(0.17, 0.6),
                max_distance: Curve::linear(0.4, 1.0),
                backstop: None,
                self_collision: false,
                ..cloth
            },
            Preset::LongHair => hair,
            Preset::ShortHair => Style {
                gravity: 0.2,
                gravity_falloff: 1.0,
                restore: Curve::linear(0.02, 0.005),
                limit: Curve::linear(0.157, 0.785),
                limit_stiffness: 1.0,
                inertia: Inertia {
                    depth_carried: 1.0,
                    speed_limit: 1.0,
                    ..hair.inertia.clone()
                },
                max_speed: 4.0,
                attenuation: 0.6,
                ..hair
            },
            Preset::Bangs => Style {
                gravity: 0.4,
                gravity_falloff: 0.8,
                restore: Curve::linear(0.025, 0.007),
                limit: Curve::linear(0.02, 0.26),
                limit_stiffness: 0.5,
                max_distance: Curve::linear(0.3, 0.6),
                max_speed: 2.0,
                ..hair
            },
            Preset::Tail => Style {
                gravity: 0.0,
                damping: Curve::flat(0.08),
                bend: 0.5,
                restore: Curve::linear(0.01, 0.001),
                limit: Curve::flat(PI),
                limit_stiffness: 0.0,
                max_speed: 3.2,
                ..hair
            },
        }
    }
}

fn cloth() -> Style {
    Style {
        gravity: 0.5,
        gravity_falloff: 0.0,
        damping: Curve::flat(0.1),
        settling: 0.0,
        drag: 0.3,
        lift: 0.1,
        stretch: 1.0,
        compression: 0.30,
        bend: 0.4,
        buckling: 0.35,
        restore: Curve::linear(0.025, 0.005),
        limit: Curve::linear(0.21, 1.05),
        limit_stiffness: 1.0,
        pull: Curve::linear(0.02, 0.0),
        max_distance: Curve::linear(0.6, 1.4),
        backstop: Some(0.02),
        tether: 1.03,
        extent: 0.0,
        inertia: Inertia {
            carried: 0.3,
            carried_turn: 0.3,
            depth_carried: 0.7,
            centrifugal: 0.1,
            speed_limit: 3.0,
            turn_limit: TAU,
            smoothing: 0.4,
        },
        max_speed: 4.0,
        attenuation: 0.5,
        friction: 0.05,
        thickness: 0.004,
        self_collision: true,
        flutter: Flutter {
            amplitude: 0.0,
            frequency: 1.5,
            wavelength: 0.3,
        },
    }
}

fn skirt(cloth: &Style) -> Style {
    Style {
        gravity: 0.15,
        gravity_falloff: 0.5,
        damping: Curve::linear(0.03, 0.013),
        settling: 0.0,
        drag: 1.0,
        lift: 0.6,
        compression: 0.26,
        bend: 0.27,
        buckling: 0.30,
        restore: Curve::from_points([[0.0, 0.035], [0.28, 0.005], [0.65, 0.001], [1.0, 0.0004]]),
        limit: Curve::linear(0.32, 1.65),
        limit_stiffness: 0.45,
        pull: Curve::from_points([[0.0, 0.02], [0.3, 0.003], [1.0, 0.0]]),
        max_distance: Curve::linear(0.9, 1.85),
        backstop: Some(0.04),
        extent: 0.84,
        inertia: Inertia {
            carried: 0.03,
            carried_turn: 0.0,
            depth_carried: 0.06,
            centrifugal: 0.85,
            smoothing: 0.16,
            ..cloth.inertia.clone()
        },
        attenuation: 0.20,
        flutter: Flutter {
            amplitude: 0.002,
            frequency: 1.6,
            wavelength: 0.28,
        },
        ..cloth.clone()
    }
}

fn hair() -> Style {
    Style {
        gravity: 0.5,
        gravity_falloff: 0.0,
        damping: Curve::flat(0.1),
        settling: 0.0,
        drag: 0.2,
        lift: 0.05,
        stretch: 1.0,
        compression: 0.5,
        bend: 0.6,
        buckling: 0.5,
        restore: Curve::linear(0.025, 0.003),
        limit: Curve::linear(0.26, 1.31),
        limit_stiffness: 0.3,
        pull: Curve::linear(0.01, 0.0),
        max_distance: Curve::linear(0.6, 1.2),
        backstop: None,
        tether: 1.02,
        extent: 0.0,
        inertia: Inertia {
            carried: 0.3,
            carried_turn: 0.3,
            depth_carried: 0.0,
            centrifugal: 0.0,
            speed_limit: 3.0,
            turn_limit: TAU,
            smoothing: 0.4,
        },
        max_speed: 3.0,
        attenuation: 0.7,
        friction: 0.05,
        thickness: 0.006,
        self_collision: false,
        flutter: Flutter {
            amplitude: 0.0,
            frequency: 1.5,
            wavelength: 0.3,
        },
    }
}
