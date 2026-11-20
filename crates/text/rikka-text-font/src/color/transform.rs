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

    pub fn translate(dx: f32, dy: f32) -> Self {
        Self {
            dx,
            dy,
            ..Self::IDENTITY
        }
    }

    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            xx: sx,
            yy: sy,
            ..Self::IDENTITY
        }
    }

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

    pub fn skew(x_degrees: f32, y_degrees: f32) -> Self {
        Self {
            xy: -x_degrees.to_radians().tan(),
            yx: y_degrees.to_radians().tan(),
            ..Self::IDENTITY
        }
    }

    pub fn around(self, center: Point) -> Self {
        Self::translate(-center.x, -center.y)
            .then(self)
            .then(Self::translate(center.x, center.y))
    }

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

    pub fn inverse(&self) -> Option<Self> {
        let determinant = self.xx * self.yy - self.xy * self.yx;
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
    pub fn apply(&self, point: Point) -> Point {
        Point::new(
            self.xx * point.x + self.xy * point.y + self.dx,
            self.yx * point.x + self.yy * point.y + self.dy,
        )
    }
}
