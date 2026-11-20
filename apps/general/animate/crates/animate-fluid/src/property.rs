use crate::Liquid;
use crate::kind::{ValueKind, amount};
use crate::value::EmitterValue;

const SHARE: ValueKind = amount(0.0, 1.0);

trait Field {
    fn read(&self) -> EmitterValue;
    fn write(&mut self, value: EmitterValue);
}

impl Field for f32 {
    fn read(&self) -> EmitterValue {
        EmitterValue::Number(*self)
    }

    fn write(&mut self, value: EmitterValue) {
        if let Some(number) = value.number() {
            *self = number;
        }
    }
}

impl Field for [f32; 3] {
    fn read(&self) -> EmitterValue {
        EmitterValue::Three(*self)
    }

    fn write(&mut self, value: EmitterValue) {
        if let Some(three) = value.three() {
            *self = three;
        }
    }
}

macro_rules! properties {
    ($($name:ident: $id:literal, $kind:expr, $field:ident;)+) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Property {
            $($name),+
        }

        impl Property {
            pub const ALL: &'static [Property] = &[$(Property::$name),+];

            #[must_use]
            pub fn id(self) -> &'static str {
                match self {
                    $(Property::$name => $id),+
                }
            }

            #[must_use]
            pub fn kind(self) -> ValueKind {
                match self {
                    $(Property::$name => $kind),+
                }
            }

            #[must_use]
            pub fn read(self, liquid: &Liquid) -> EmitterValue {
                match self {
                    $(Property::$name => Field::read(&liquid.$field)),+
                }
            }

            pub fn write(self, liquid: &mut Liquid, value: EmitterValue) {
                match self {
                    $(Property::$name => Field::write(&mut liquid.$field, value)),+
                }
            }
        }
    };
}

properties! {
    Viscosity: "viscosity", SHARE, viscosity;
    Elasticity: "elasticity", SHARE, elasticity;
    Relaxation: "relaxation", SHARE, relaxation;
    Cohesion: "cohesion", SHARE, cohesion;
    Adhesion: "adhesion", SHARE, adhesion;
    Friction: "friction", SHARE, friction;
    Spray: "spray", SHARE, spray;
    Wetting: "wetting", SHARE, wetting;
    Color: "color", ValueKind::Color, color;
    Depth: "depth", amount(0.001, 10.0), depth;
    Cloudiness: "cloudiness", SHARE, cloudiness;
    Roughness: "roughness", SHARE, roughness;
    Ior: "ior", amount(1.0, 2.0), ior;
    Tint: "tint", ValueKind::Color, tint;
}

impl Property {
    #[must_use]
    pub fn named(id: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|property| property.id() == id)
    }
}
