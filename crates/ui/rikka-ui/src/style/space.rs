#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Space(f32);

impl Space {
    pub const NONE: Self = Self(0.0);
    pub const HAIR: Self = Self(2.0);
    pub const TIGHT: Self = Self(4.0);
    pub const SNUG: Self = Self(8.0);
    pub const NORMAL: Self = Self(12.0);
    pub const ROOMY: Self = Self(16.0);
    pub const LOOSE: Self = Self(24.0);
    pub const WIDE: Self = Self(32.0);

    pub const fn px(self) -> f32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Inset {
    top: f32,
    right: f32,
    bottom: f32,
    left: f32,
}

impl Inset {
    pub const NONE: Self = Self::exact(0.0, 0.0, 0.0, 0.0);

    pub fn all(space: impl Into<Space>) -> Self {
        let step = space.into().px();
        Self::exact(step, step, step, step)
    }

    pub fn xy(x: impl Into<Space>, y: impl Into<Space>) -> Self {
        let (x, y) = (x.into().px(), y.into().px());
        Self::exact(y, x, y, x)
    }

    pub fn edges(
        top: impl Into<Space>,
        right: impl Into<Space>,
        bottom: impl Into<Space>,
        left: impl Into<Space>,
    ) -> Self {
        Self::exact(
            top.into().px(),
            right.into().px(),
            bottom.into().px(),
            left.into().px(),
        )
    }

    pub const fn exact(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top: sane(top),
            right: sane(right),
            bottom: sane(bottom),
            left: sane(left),
        }
    }

    pub const fn top(&self) -> f32 {
        self.top
    }

    pub const fn right(&self) -> f32 {
        self.right
    }

    pub const fn bottom(&self) -> f32 {
        self.bottom
    }

    pub const fn left(&self) -> f32 {
        self.left
    }

    pub const fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    pub const fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Length(Measure);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Measure {
    #[default]
    Hug,
    Px(f32),
    Portion(f32),
}

impl Length {
    pub const HUG: Self = Self(Measure::Hug);

    pub const FILL: Self = Self(Measure::Portion(1.0));

    pub fn portion(share: f32) -> Self {
        Self(Measure::Portion(sane(share).min(1.0)))
    }

    pub const fn px(px: f32) -> Self {
        Self(Measure::Px(sane(px)))
    }

    pub fn resolve(self, available: f32) -> Option<f32> {
        match self.0 {
            Measure::Hug => None,
            Measure::Px(px) => Some(px),
            Measure::Portion(share) => Some(available.max(0.0) * share),
        }
    }

    pub fn fixed(self) -> Option<f32> {
        match self.0 {
            Measure::Px(px) => Some(px),
            Measure::Hug | Measure::Portion(_) => None,
        }
    }
}

macro_rules! from_measures {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl From<$ty> for Space {
                fn from(px: $ty) -> Self {
                    Self(sane(px as f32))
                }
            }

            impl From<$ty> for Inset {
                fn from(px: $ty) -> Self {
                    Self::all(px)
                }
            }

            impl From<[$ty; 2]> for Inset {
                fn from([x, y]: [$ty; 2]) -> Self {
                    Self::xy(x, y)
                }
            }

            impl From<[$ty; 4]> for Inset {
                fn from([top, right, bottom, left]: [$ty; 4]) -> Self {
                    Self::edges(top, right, bottom, left)
                }
            }

            impl From<$ty> for Length {
                fn from(px: $ty) -> Self {
                    Self::px(px as f32)
                }
            }
        )+
    };
}

from_measures!(f32, f64);

impl From<Space> for Inset {
    fn from(space: Space) -> Self {
        Self::all(space)
    }
}

impl From<[Space; 2]> for Inset {
    fn from([x, y]: [Space; 2]) -> Self {
        Self::xy(x, y)
    }
}

impl From<Space> for Length {
    fn from(space: Space) -> Self {
        Self::px(space.px())
    }
}

const fn sane(value: f32) -> f32 {
    match value.is_finite() && value > 0.0 {
        true => value,
        false => 0.0,
    }
}
