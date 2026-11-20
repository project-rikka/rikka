use crate::parse::bytes::Bytes;
use crate::parse::f2dot14::F2Dot14;
use crate::variations::fixed::Fixed;
use crate::variations::item_variation_store::axis_scalar;

pub(crate) trait TupleScalar: Copy {
    fn compute(
        axis_count: usize,
        peak: &[u8],
        intermediate: Option<(&[u8], &[u8])>,
        coords: &[F2Dot14],
    ) -> Option<Self>;

    fn is_zero(self) -> bool;
}

impl TupleScalar for f32 {
    fn compute(
        axis_count: usize,
        peak: &[u8],
        intermediate: Option<(&[u8], &[u8])>,
        coords: &[F2Dot14],
    ) -> Option<Self> {
        let mut scalar = 1.0;
        for axis in 0..axis_count {
            let peak_value = peak.i16_at(axis * 2)?;
            if peak_value == 0 {
                continue;
            }
            let coord = coords.get(axis).map_or(0, |c| c.0);
            let (start, end) = match intermediate {
                Some((starts, ends)) => (starts.i16_at(axis * 2)?, ends.i16_at(axis * 2)?),
                None => (peak_value.min(0), peak_value.max(0)),
            };
            let factor = axis_scalar(start, peak_value, end, coord);
            if factor == 0.0 {
                return Some(0.0);
            }
            scalar *= factor;
        }
        Some(scalar)
    }

    #[inline]
    fn is_zero(self) -> bool {
        self == 0.0
    }
}

impl TupleScalar for Fixed {
    fn compute(
        axis_count: usize,
        peak: &[u8],
        intermediate: Option<(&[u8], &[u8])>,
        coords: &[F2Dot14],
    ) -> Option<Self> {
        let mut scalar = Fixed::ONE;
        for axis in 0..axis_count {
            let peak = peak.i16_at(axis * 2)?;
            if peak == 0 {
                continue;
            }
            let coord = coords.get(axis).map_or(0, |c| c.0);
            if coord == 0 {
                return Some(Fixed::ZERO);
            }
            if coord == peak {
                continue;
            }
            match intermediate {
                Some((starts, ends)) => {
                    let start = starts.i16_at(axis * 2)?;
                    let end = ends.i16_at(axis * 2)?;
                    if coord <= start || coord >= end {
                        return Some(Fixed::ZERO);
                    }
                    let [start, peak, end, coord] =
                        [start, peak, end, coord].map(Fixed::from_f2dot14);
                    scalar = if coord.0 < peak.0 {
                        scalar.mul_div(coord.sub(start), peak.sub(start))
                    } else {
                        scalar.mul_div(end.sub(coord), end.sub(peak))
                    };
                }
                None => {
                    if coord < peak.min(0) || coord > peak.max(0) {
                        return Some(Fixed::ZERO);
                    }
                    scalar = scalar.mul_div(Fixed::from_f2dot14(coord), Fixed::from_f2dot14(peak));
                }
            }
        }
        Some(scalar)
    }

    #[inline]
    fn is_zero(self) -> bool {
        self == Fixed::ZERO
    }
}
