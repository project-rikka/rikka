#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Fixed(pub(crate) i32);

impl Fixed {
    pub(crate) const ZERO: Self = Self(0);
    pub(crate) const ONE: Self = Self(0x10000);

    #[inline]
    pub(crate) fn from_i32(value: i32) -> Self {
        Self(value.wrapping_shl(16))
    }

    #[inline]
    pub(crate) fn from_f2dot14(value: i16) -> Self {
        Self(i32::from(value) * 4)
    }

    #[inline]
    pub(crate) fn rounded(self) -> i32 {
        self.0.wrapping_add(0x8000) >> 16
    }

    #[inline]
    pub(crate) fn mul(self, other: Self) -> Self {
        let product = i64::from(self.0) * i64::from(other.0);
        Self(((product + 0x8000 - i64::from(product < 0)) >> 16) as i32)
    }

    #[inline]
    pub(crate) fn add(self, other: Self) -> Self {
        Self(self.0.wrapping_add(other.0))
    }

    #[inline]
    pub(crate) fn sub(self, other: Self) -> Self {
        Self(self.0.wrapping_sub(other.0))
    }

    pub(crate) fn mul_div(self, numerator: Self, denominator: Self) -> Self {
        let negative = (self.0 < 0) ^ (numerator.0 < 0) ^ (denominator.0 < 0);
        let a = u64::from(self.0.unsigned_abs());
        let b = u64::from(numerator.0.unsigned_abs());
        let c = u64::from(denominator.0.unsigned_abs());
        let result = a
            .wrapping_mul(b)
            .wrapping_add(c >> 1)
            .checked_div(c)
            .map_or(0x7FFF_FFFF, |quotient| quotient as i32);
        Self(if negative {
            result.wrapping_neg()
        } else {
            result
        })
    }
}
