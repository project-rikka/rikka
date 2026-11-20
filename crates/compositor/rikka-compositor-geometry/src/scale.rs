use std::ops::Mul;

use crate::Coordinate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Scale<N> {
    pub x: N,
    pub y: N,
}

impl<N: Coordinate> Scale<N> {
    pub fn to_f64(self) -> Scale<f64> {
        Scale {
            x: self.x.to_f64(),
            y: self.y.to_f64(),
        }
    }
}

impl<N: Coordinate> From<N> for Scale<N> {
    fn from(scale: N) -> Self {
        Scale { x: scale, y: scale }
    }
}

impl<N: Coordinate> From<(N, N)> for Scale<N> {
    fn from((x, y): (N, N)) -> Self {
        Scale { x, y }
    }
}

impl<N: Coordinate, T: Into<Scale<N>>> Mul<T> for Scale<N> {
    type Output = Scale<N>;

    fn mul(self, rhs: T) -> Scale<N> {
        let rhs = rhs.into();
        Scale {
            x: self.x.upscale(rhs.x),
            y: self.y.upscale(rhs.y),
        }
    }
}
