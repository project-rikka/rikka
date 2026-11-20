use std::fmt::Debug;
use std::hash::Hash;

pub trait Frame: Copy + Ord + Hash + Debug + Send + Sync + 'static {
    const ZERO: Self;

    fn to_f64(self) -> f64;

    fn from_f64(value: f64) -> Self;

    #[must_use]
    fn offset(self, by: i64) -> Self;
}

impl Frame for u32 {
    const ZERO: Self = 0;

    fn to_f64(self) -> f64 {
        f64::from(self)
    }

    fn from_f64(value: f64) -> Self {
        value.round().clamp(0.0, f64::from(u32::MAX)) as u32
    }

    fn offset(self, by: i64) -> Self {
        u32::try_from((i64::from(self) + by).max(0)).unwrap_or(u32::MAX)
    }
}

impl Frame for i64 {
    const ZERO: Self = 0;

    fn to_f64(self) -> f64 {
        self as f64
    }

    fn from_f64(value: f64) -> Self {
        value.round() as i64
    }

    fn offset(self, by: i64) -> Self {
        self.saturating_add(by)
    }
}

pub trait Framed {
    type Frame: Frame;

    fn frame(&self) -> Self::Frame;
}
