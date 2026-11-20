use rikka_types::Point;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub xx: f64,
    pub xy: f64,
    pub yx: f64,
    pub yy: f64,
    pub tx: f64,
    pub ty: f64,
}

impl Default for Affine {
    fn default() -> Affine {
        Affine::IDENTITY
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

    #[must_use]
    pub fn translation(x: f64, y: f64) -> Affine {
        Affine {
            tx: x,
            ty: y,
            ..Affine::IDENTITY
        }
    }

    #[must_use]
    pub fn scale(x: f64, y: f64) -> Affine {
        Affine {
            xx: x,
            yy: y,
            ..Affine::IDENTITY
        }
    }

    #[must_use]
    pub fn rotation(clockwise_degrees: f64) -> Affine {
        let (sin, cos) = clockwise_degrees.to_radians().sin_cos();
        Affine {
            xx: cos,
            xy: sin,
            yx: -sin,
            yy: cos,
            tx: 0.0,
            ty: 0.0,
        }
    }

    #[must_use]
    pub fn about(center: Point, map: Affine) -> Affine {
        Affine::translation(-f64::from(center.x), -f64::from(center.y))
            .then(map)
            .then(Affine::translation(
                f64::from(center.x),
                f64::from(center.y),
            ))
    }

    #[must_use]
    pub fn from_array([xx, xy, yx, yy, tx, ty]: [f64; 6]) -> Affine {
        Affine {
            xx,
            xy,
            yx,
            yy,
            tx,
            ty,
        }
    }

    #[must_use]
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

    #[must_use]
    pub fn apply(self, at: Point) -> Point {
        let (x, y) = (f64::from(at.x), f64::from(at.y));
        Point::new(
            (self.xx * x + self.yx * y + self.tx) as f32,
            (self.xy * x + self.yy * y + self.ty) as f32,
        )
    }

    #[must_use]
    pub fn apply_vector(self, along: Point) -> Point {
        let (x, y) = (f64::from(along.x), f64::from(along.y));
        Point::new(
            (self.xx * x + self.yx * y) as f32,
            (self.xy * x + self.yy * y) as f32,
        )
    }

    #[must_use]
    pub fn determinant(self) -> f64 {
        self.xx * self.yy - self.yx * self.xy
    }

    #[must_use]
    pub fn stretch(self) -> f32 {
        self.determinant().abs().sqrt() as f32
    }

    #[must_use]
    pub fn is_translation(self) -> bool {
        self.xx == 1.0 && self.yy == 1.0 && self.xy == 0.0 && self.yx == 0.0
    }

    #[must_use]
    pub fn inverse(self) -> Option<Affine> {
        let determinant = self.determinant();
        if !determinant.is_normal() || !self.to_array().iter().all(|value| value.is_finite()) {
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
}
