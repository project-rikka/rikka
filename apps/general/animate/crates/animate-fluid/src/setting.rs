use crate::aim::Aim;
use crate::kind::{ValueKind, amount, angle};
use crate::shape::Shape;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EmitterSection {
    Emission,
    Path,
    Shape,
}

impl EmitterSection {
    pub const ALL: [EmitterSection; 3] = [Self::Emission, Self::Path, Self::Shape];

    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Emission => "emission",
            Self::Path => "path",
            Self::Shape => "shape",
        }
    }

    pub fn settings(self) -> impl Iterator<Item = EmitterSetting> {
        EmitterSetting::ALL
            .iter()
            .copied()
            .filter(move |setting| setting.section() == self)
    }
}

macro_rules! settings {
    ($($name:ident: $id:literal, $section:ident, $kind:expr;)+) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum EmitterSetting {
            $($name),+
        }

        impl EmitterSetting {
            pub const ALL: &'static [EmitterSetting] = &[$(EmitterSetting::$name),+];

            #[must_use]
            pub fn id(self) -> &'static str {
                match self {
                    $(EmitterSetting::$name => $id),+
                }
            }

            #[must_use]
            pub fn section(self) -> EmitterSection {
                match self {
                    $(EmitterSetting::$name => EmitterSection::$section),+
                }
            }

            #[must_use]
            pub fn kind(self) -> ValueKind {
                match self {
                    $(EmitterSetting::$name => $kind),+
                }
            }
        }
    };
}

settings! {
    Rate: "rate", Emission, amount(0.0, 2.0);
    Volume: "volume", Emission, amount(0.001, 2.0);
    Launch: "launch", Path, ValueKind::Position;
    Target: "target", Path, ValueKind::Position;
    Speed: "speed", Path, amount(0.0, 20.0);
    Arc: "arc", Path, amount(0.0, 5.0);
    Width: "width", Shape, amount(0.001, 0.2);
    Spread: "spread", Shape, angle(0.0, 120.0);
}

impl EmitterSetting {
    #[must_use]
    pub fn named(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|setting| setting.id() == id)
    }

    #[must_use]
    pub fn applies(self, shape: &Shape, aim: Aim) -> bool {
        match self {
            Self::Rate => shape.flows(),
            Self::Volume => matches!(shape, Shape::Blob { .. }),
            Self::Launch => shape.launches(),
            Self::Target => shape.aims(),
            Self::Speed => shape.aims() && aim == Aim::Direction,
            Self::Arc => shape.aims() && aim == Aim::Ballistic,
            Self::Width => matches!(shape, Shape::Stream | Shape::Shower | Shape::Drip),
            Self::Spread => matches!(shape, Shape::Shower),
        }
    }
}
