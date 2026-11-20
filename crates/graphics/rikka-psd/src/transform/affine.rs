const EPSILON: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine {
    pub xx: f64,
    pub xy: f64,
    pub yx: f64,
    pub yy: f64,
    pub tx: f64,
    pub ty: f64,
}

impl Default for Affine {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Affine {
    pub const IDENTITY: Affine = Affine {
        xx: 1.0,
        xy: 0.0,
        yx: 0.0,
        yy: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    pub fn translation(x: f64, y: f64) -> Self {
        Self {
            tx: x,
            ty: y,
            ..Self::IDENTITY
        }
    }

    pub fn scale(x: f64, y: f64) -> Self {
        Self {
            xx: x,
            yy: y,
            ..Self::IDENTITY
        }
    }

    pub fn rotation(clockwise_degrees: f64) -> Self {
        let (sin, cos) = clockwise_degrees.to_radians().sin_cos();
        Self {
            xx: cos,
            xy: sin,
            yx: -sin,
            yy: cos,
            tx: 0.0,
            ty: 0.0,
        }
    }

    pub fn from_array([xx, xy, yx, yy, tx, ty]: [f64; 6]) -> Self {
        Self {
            xx,
            xy,
            yx,
            yy,
            tx,
            ty,
        }
    }

    pub fn to_array(self) -> [f64; 6] {
        [self.xx, self.xy, self.yx, self.yy, self.tx, self.ty]
    }

    #[must_use]
    pub fn then(self, next: Affine) -> Affine {
        Affine {
            xx: next.xx * self.xx + next.yx * self.xy,
            xy: next.xy * self.xx + next.yy * self.xy,
            yx: next.xx * self.yx + next.yx * self.yy,
            yy: next.xy * self.yx + next.yy * self.yy,
            tx: next.xx * self.tx + next.yx * self.ty + next.tx,
            ty: next.xy * self.tx + next.yy * self.ty + next.ty,
        }
    }

    pub fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (
            self.xx * x + self.yx * y + self.tx,
            self.xy * x + self.yy * y + self.ty,
        )
    }

    pub fn apply_vector(self, x: f64, y: f64) -> (f64, f64) {
        (self.xx * x + self.yx * y, self.xy * x + self.yy * y)
    }

    pub fn determinant(self) -> f64 {
        self.xx * self.yy - self.yx * self.xy
    }

    pub fn inverse(self) -> Option<Affine> {
        let determinant = self.determinant();
        if !determinant.is_normal() || !self.is_finite() {
            return None;
        }
        let xx = self.yy / determinant;
        let xy = -self.xy / determinant;
        let yx = -self.yx / determinant;
        let yy = self.xx / determinant;
        Some(Affine {
            xx,
            xy,
            yx,
            yy,
            tx: -(xx * self.tx + yx * self.ty),
            ty: -(xy * self.tx + yy * self.ty),
        })
    }

    pub fn is_finite(self) -> bool {
        self.to_array().iter().all(|value| value.is_finite())
    }

    pub fn is_axis_aligned(self) -> bool {
        self.xy.abs() < EPSILON && self.yx.abs() < EPSILON
    }

    pub fn swaps_axes(self) -> bool {
        self.xx.abs() < EPSILON && self.yy.abs() < EPSILON
    }

    pub(crate) fn mean_scale(self) -> f64 {
        self.determinant().abs().sqrt()
    }
}
