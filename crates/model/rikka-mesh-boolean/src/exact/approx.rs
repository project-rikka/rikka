use std::ops::{Add, Mul, Sub};

const ROUNDING: f64 = f64::EPSILON;
const SAFETY: f64 = 1.0 + 1.0e-9;
const UNDERFLOW: f64 = 1.0e-280;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Approx {
    value: f64,
    error: f64,
}

impl Approx {
    pub(crate) fn new(value: f64, error: f64) -> Approx {
        Approx { value, error }
    }

    pub(crate) fn sign(self) -> Option<i32> {
        let magnitude = self.value.abs();
        if !magnitude.is_finite() || !self.error.is_finite() {
            return None;
        }
        match magnitude > self.error * SAFETY && magnitude > UNDERFLOW {
            true => Some(if self.value > 0.0 { 1 } else { -1 }),
            false => None,
        }
    }

    pub(crate) fn exact_zero(self) -> bool {
        self.value == 0.0 && self.error == 0.0
    }
}

impl Add for Approx {
    type Output = Approx;

    fn add(self, other: Approx) -> Approx {
        let value = self.value + other.value;
        Approx {
            value,
            error: self.error + other.error + value.abs() * ROUNDING,
        }
    }
}

impl Sub for Approx {
    type Output = Approx;

    fn sub(self, other: Approx) -> Approx {
        let value = self.value - other.value;
        Approx {
            value,
            error: self.error + other.error + value.abs() * ROUNDING,
        }
    }
}

impl Mul for Approx {
    type Output = Approx;

    fn mul(self, other: Approx) -> Approx {
        let value = self.value * other.value;
        Approx {
            value,
            error: self.value.abs() * other.error
                + other.value.abs() * self.error
                + self.error * other.error
                + value.abs() * ROUNDING,
        }
    }
}
