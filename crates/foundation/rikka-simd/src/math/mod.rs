mod constants;
mod exp;
mod inverse;
mod kernel;
mod log;
mod pow;
mod trig;

pub(crate) use constants::MathElement;

use crate::Float;

pub trait FloatMath: Float {
    fn exp(self) -> Self;

    fn exp2(self) -> Self;

    fn ln(self) -> Self;

    fn log2(self) -> Self;

    fn log10(self) -> Self;

    fn pow(self, exponent: Self) -> Self;

    fn sin(self) -> Self;

    fn cos(self) -> Self;

    fn sin_cos(self) -> (Self, Self);

    fn tan(self) -> Self;

    fn asin(self) -> Self;

    fn acos(self) -> Self;

    fn atan(self) -> Self;

    fn atan2(self, x: Self) -> Self;
}

impl<V: Float> FloatMath for V {
    #[inline(always)]
    fn exp(self) -> Self {
        exp::exp(self)
    }

    #[inline(always)]
    fn exp2(self) -> Self {
        exp::exp2(self)
    }

    #[inline(always)]
    fn ln(self) -> Self {
        log::ln(self)
    }

    #[inline(always)]
    fn log2(self) -> Self {
        log::log2(self)
    }

    #[inline(always)]
    fn log10(self) -> Self {
        log::log10(self)
    }

    #[inline(always)]
    fn pow(self, exponent: Self) -> Self {
        pow::pow(self, exponent)
    }

    #[inline(always)]
    fn sin(self) -> Self {
        trig::sin(self)
    }

    #[inline(always)]
    fn cos(self) -> Self {
        trig::cos(self)
    }

    #[inline(always)]
    fn sin_cos(self) -> (Self, Self) {
        trig::sin_cos(self)
    }

    #[inline(always)]
    fn tan(self) -> Self {
        trig::tan(self)
    }

    #[inline(always)]
    fn asin(self) -> Self {
        inverse::asin(self)
    }

    #[inline(always)]
    fn acos(self) -> Self {
        inverse::acos(self)
    }

    #[inline(always)]
    fn atan(self) -> Self {
        inverse::atan(self)
    }

    #[inline(always)]
    fn atan2(self, x: Self) -> Self {
        inverse::atan2(self, x)
    }
}
