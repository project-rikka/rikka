use std::f32::consts::{PI, TAU};
use std::ops::RangeInclusive;

use rikka_physics_tailor::{Curve, Style};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldSection {
    Motion,
    Shape,
    Return,
    Inertia,
    Contact,
    Flutter,
}

impl FieldSection {
    pub const ALL: [FieldSection; 6] = [
        FieldSection::Motion,
        FieldSection::Shape,
        FieldSection::Return,
        FieldSection::Inertia,
        FieldSection::Contact,
        FieldSection::Flutter,
    ];

    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            FieldSection::Motion => "motion",
            FieldSection::Shape => "shape",
            FieldSection::Return => "return",
            FieldSection::Inertia => "inertia",
            FieldSection::Contact => "contact",
            FieldSection::Flutter => "flutter",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldKind {
    Number,
    Curve,
    Toggle,
    Optional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldUnit {
    Plain,
    Radians,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Reading {
    Number(f32),
    Curve(Curve),
    Toggle(bool),
    Optional(Option<f32>),
}

macro_rules! fields {
    ($($name:ident: $id:literal, $section:ident, $kind:ident, $unit:ident, $min:expr, $max:expr, $($path:ident).+;)+) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Field {
            $($name),+
        }

        impl Field {
            pub const ALL: &'static [Field] = &[$(Field::$name),+];

            #[must_use]
            pub fn id(self) -> &'static str {
                match self {
                    $(Field::$name => $id),+
                }
            }

            #[must_use]
            pub fn section(self) -> FieldSection {
                match self {
                    $(Field::$name => FieldSection::$section),+
                }
            }

            #[must_use]
            pub fn kind(self) -> FieldKind {
                match self {
                    $(Field::$name => FieldKind::$kind),+
                }
            }

            #[must_use]
            pub fn unit(self) -> FieldUnit {
                match self {
                    $(Field::$name => FieldUnit::$unit),+
                }
            }

            #[must_use]
            pub fn range(self) -> RangeInclusive<f32> {
                match self {
                    $(Field::$name => $min..=$max),+
                }
            }

            #[must_use]
            pub fn read(self, style: &Style) -> Reading {
                match self {
                    $(Field::$name => Reading::$kind(Clone::clone(&style.$($path).+))),+
                }
            }

            pub fn write(self, style: &mut Style, reading: Reading) {
                match (self, reading) {
                    $((Field::$name, Reading::$kind(value)) => style.$($path).+ = value,)+
                    _ => {}
                }
            }
        }
    };
}

fields! {
    Gravity: "gravity", Motion, Number, Plain, 0.0, 2.0, gravity;
    GravityFalloff: "gravity_falloff", Motion, Number, Plain, 0.0, 1.0, gravity_falloff;
    Damping: "damping", Motion, Curve, Plain, 0.0, 0.95, damping;
    Settling: "settling", Motion, Number, Plain, 0.0, 0.95, settling;
    Drag: "drag", Motion, Number, Plain, 0.0, 1.0, drag;
    Lift: "lift", Motion, Number, Plain, 0.0, 1.0, lift;
    MaxSpeed: "max_speed", Motion, Number, Plain, 0.5, 12.0, max_speed;
    Attenuation: "attenuation", Motion, Number, Plain, 0.05, 0.95, attenuation;
    Stretch: "stretch", Shape, Number, Plain, 0.0, 1.0, stretch;
    Compression: "compression", Shape, Number, Plain, 0.0, 1.0, compression;
    Bend: "bend", Shape, Number, Plain, 0.0, 1.0, bend;
    Buckling: "buckling", Shape, Number, Plain, 0.0, 1.0, buckling;
    Tether: "tether", Shape, Number, Plain, 1.0, 1.1, tether;
    Restore: "restore", Return, Curve, Plain, 0.0, 0.3, restore;
    Limit: "limit", Return, Curve, Radians, 0.0, PI, limit;
    LimitStiffness: "limit_stiffness", Return, Number, Plain, 0.0, 1.0, limit_stiffness;
    Pull: "pull", Return, Curve, Plain, 0.0, 0.1, pull;
    MaxDistance: "max_distance", Return, Curve, Plain, 0.0, 3.0, max_distance;
    Backstop: "backstop", Return, Optional, Plain, 0.0, 0.05, backstop;
    Carried: "carried", Inertia, Number, Plain, -1.0, 1.0, inertia.carried;
    CarriedTurn: "carried_turn", Inertia, Number, Plain, -1.0, 1.0, inertia.carried_turn;
    DepthCarried: "depth_carried", Inertia, Number, Plain, 0.0, 1.0, inertia.depth_carried;
    Centrifugal: "centrifugal", Inertia, Number, Plain, 0.0, 2.0, inertia.centrifugal;
    SpeedLimit: "speed_limit", Inertia, Number, Plain, 0.0, 12.0, inertia.speed_limit;
    TurnLimit: "turn_limit", Inertia, Number, Radians, 0.0, 4.0 * TAU, inertia.turn_limit;
    Smoothing: "smoothing", Inertia, Number, Plain, 0.0, 1.0, inertia.smoothing;
    Friction: "friction", Contact, Number, Plain, 0.0, 1.0, friction;
    Thickness: "thickness", Contact, Number, Plain, 0.0005, 0.02, thickness;
    SelfCollision: "self_collision", Contact, Toggle, Plain, 0.0, 1.0, self_collision;
    Amplitude: "amplitude", Flutter, Number, Plain, 0.0, 0.02, flutter.amplitude;
    Frequency: "frequency", Flutter, Number, Plain, 0.0, 5.0, flutter.frequency;
    Wavelength: "wavelength", Flutter, Number, Plain, 0.05, 2.0, flutter.wavelength;
}

impl Field {
    pub fn for_section(section: FieldSection) -> impl Iterator<Item = Field> {
        Field::ALL
            .iter()
            .copied()
            .filter(move |field| field.section() == section)
    }
}
