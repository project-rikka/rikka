use rikka_types::Point;

const SINGULAR_DETERMINANT: f32 = 1.0e-12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub xx: f32,
    pub yx: f32,
    pub xy: f32,
    pub yy: f32,
    pub dx: f32,
    pub dy: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Self = Self {
        xx: 1.0,
        yx: 0.0,
        xy: 0.0,
        yy: 1.0,
        dx: 0.0,
        dy: 0.0,
    };

    #[must_use]
    pub const fn new(xx: f32, yx: f32, xy: f32, yy: f32, dx: f32, dy: f32) -> Self {
        Self {
            xx,
            yx,
            xy,
            yy,
            dx,
            dy,
        }
    }

    #[must_use]
    pub const fn translate(dx: f32, dy: f32) -> Self {
        Self {
            dx,
            dy,
            ..Self::IDENTITY
        }
    }

    #[must_use]
    pub const fn scale(sx: f32, sy: f32) -> Self {
        Self {
            xx: sx,
            yy: sy,
            ..Self::IDENTITY
        }
    }

    #[must_use]
    pub fn rotate(degrees: f32) -> Self {
        let (sin, cos) = degrees.to_radians().sin_cos();
        Self {
            xx: cos,
            yx: sin,
            xy: -sin,
            yy: cos,
            dx: 0.0,
            dy: 0.0,
        }
    }

    #[must_use]
    pub fn skew(x_degrees: f32, y_degrees: f32) -> Self {
        Self {
            xy: x_degrees.to_radians().tan(),
            yx: y_degrees.to_radians().tan(),
            ..Self::IDENTITY
        }
    }

    #[must_use]
    pub fn around(self, center: Point) -> Self {
        Self::translate(-center.x, -center.y)
            .then(self)
            .then(Self::translate(center.x, center.y))
    }

    #[must_use]
    pub fn then(self, next: Self) -> Self {
        Self {
            xx: next.xx * self.xx + next.xy * self.yx,
            yx: next.yx * self.xx + next.yy * self.yx,
            xy: next.xx * self.xy + next.xy * self.yy,
            yy: next.yx * self.xy + next.yy * self.yy,
            dx: next.xx * self.dx + next.xy * self.dy + next.dx,
            dy: next.yx * self.dx + next.yy * self.dy + next.dy,
        }
    }

    #[must_use]
    pub fn inverse(&self) -> Option<Self> {
        let determinant = self.determinant();
        if determinant.abs() < SINGULAR_DETERMINANT || !determinant.is_finite() {
            return None;
        }
        let scale = determinant.recip();
        let xx = self.yy * scale;
        let xy = -self.xy * scale;
        let yx = -self.yx * scale;
        let yy = self.xx * scale;
        Some(Self {
            xx,
            yx,
            xy,
            yy,
            dx: -(xx * self.dx + xy * self.dy),
            dy: -(yx * self.dx + yy * self.dy),
        })
    }

    #[inline]
    #[must_use]
    pub fn apply(&self, point: Point) -> Point {
        Point::new(
            self.xx * point.x + self.xy * point.y + self.dx,
            self.yx * point.x + self.yy * point.y + self.dy,
        )
    }

    #[inline]
    #[must_use]
    pub fn apply_vector(&self, vector: Point) -> Point {
        Point::new(
            self.xx * vector.x + self.xy * vector.y,
            self.yx * vector.x + self.yy * vector.y,
        )
    }

    #[inline]
    #[must_use]
    pub fn determinant(&self) -> f32 {
        self.xx * self.yy - self.xy * self.yx
    }

    #[must_use]
    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }

    #[must_use]
    pub fn is_finite(&self) -> bool {
        (self.xx + self.yx + self.xy + self.yy + self.dx + self.dy).is_finite()
    }

    #[must_use]
    pub fn max_scale(&self) -> f32 {
        let (a, b, c, d) = (self.xx, self.xy, self.yx, self.yy);
        let frobenius = a * a + b * b + c * c + d * d;
        let determinant = a * d - b * c;
        let spread = (frobenius * frobenius - 4.0 * determinant * determinant)
            .max(0.0)
            .sqrt();
        ((frobenius + spread) * 0.5).sqrt()
    }

    #[must_use]
    pub fn mean_scale(&self) -> f32 {
        self.determinant().abs().sqrt()
    }
}
