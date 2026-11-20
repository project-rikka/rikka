use std::hash::{Hash, Hasher};

#[derive(Clone, Copy, Debug)]
pub struct Weight(pub f32);

impl Weight {
    pub const THIN: Self = Self(100.0);
    pub const EXTRA_LIGHT: Self = Self(200.0);
    pub const LIGHT: Self = Self(300.0);
    pub const NORMAL: Self = Self(400.0);
    pub const MEDIUM: Self = Self(500.0);
    pub const SEMI_BOLD: Self = Self(600.0);
    pub const BOLD: Self = Self(700.0);
    pub const EXTRA_BOLD: Self = Self(800.0);
    pub const BLACK: Self = Self(900.0);
}

impl Default for Weight {
    fn default() -> Self {
        Self::NORMAL
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Width(pub f32);

impl Width {
    pub const ULTRA_CONDENSED: Self = Self(50.0);
    pub const EXTRA_CONDENSED: Self = Self(62.5);
    pub const CONDENSED: Self = Self(75.0);
    pub const SEMI_CONDENSED: Self = Self(87.5);
    pub const NORMAL: Self = Self(100.0);
    pub const SEMI_EXPANDED: Self = Self(112.5);
    pub const EXPANDED: Self = Self(125.0);
    pub const EXTRA_EXPANDED: Self = Self(150.0);
    pub const ULTRA_EXPANDED: Self = Self(200.0);
}

impl Default for Width {
    fn default() -> Self {
        Self::NORMAL
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique(f32),
}

impl FontStyle {
    pub const OBLIQUE: Self = Self::Oblique(14.0);

    fn key(self) -> (u8, u32) {
        match self {
            Self::Normal => (0, 0),
            Self::Italic => (1, 0),
            Self::Oblique(angle) => (2, angle.to_bits()),
        }
    }
}

macro_rules! bitwise_identity {
    ($type:ty, $key:expr) => {
        impl PartialEq for $type {
            fn eq(&self, other: &Self) -> bool {
                $key(*self) == $key(*other)
            }
        }

        impl Eq for $type {}

        impl Hash for $type {
            fn hash<H: Hasher>(&self, state: &mut H) {
                $key(*self).hash(state);
            }
        }
    };
}

bitwise_identity!(Weight, |weight: Weight| weight.0.to_bits());
bitwise_identity!(Width, |width: Width| width.0.to_bits());
bitwise_identity!(FontStyle, FontStyle::key);
