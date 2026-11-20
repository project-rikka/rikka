use rikka_figure::Affine;
use rikka_types::Point;

const SINGULAR: f64 = 1e-12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Homography {
    rows: [[f64; 3]; 3],
}

impl Default for Homography {
    fn default() -> Homography {
        Homography::IDENTITY
    }
}

impl Homography {
    pub const IDENTITY: Homography = Homography {
        rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };

    #[must_use]
    pub fn new(rows: [[f64; 3]; 3]) -> Homography {
        Homography { rows }
    }

    #[must_use]
    pub fn from_affine(affine: Affine) -> Homography {
        Homography {
            rows: [
                [affine.xx, affine.yx, affine.tx],
                [affine.xy, affine.yy, affine.ty],
                [0.0, 0.0, 1.0],
            ],
        }
    }

    #[must_use]
    pub fn about(center: Point, scale: f64) -> Homography {
        let (x, y) = (f64::from(center.x), f64::from(center.y));
        Homography {
            rows: [
                [scale, 0.0, x - scale * x],
                [0.0, scale, y - scale * y],
                [0.0, 0.0, 1.0],
            ],
        }
    }

    #[must_use]
    pub fn rows(&self) -> [[f64; 3]; 3] {
        self.rows
    }

    #[must_use]
    pub fn matrix(&self) -> [[f32; 3]; 3] {
        self.rows.map(|row| row.map(|value| value as f32))
    }

    #[must_use]
    pub fn affine(&self) -> Option<Affine> {
        let [top, middle, bottom] = self.rows;
        let flat = bottom[0].abs() < SINGULAR && bottom[1].abs() < SINGULAR && bottom[2] > SINGULAR;
        flat.then(|| {
            let scale = 1.0 / bottom[2];
            Affine {
                xx: top[0] * scale,
                yx: top[1] * scale,
                tx: top[2] * scale,
                xy: middle[0] * scale,
                yy: middle[1] * scale,
                ty: middle[2] * scale,
            }
        })
    }

    #[must_use]
    pub fn then(self, next: Homography) -> Homography {
        Homography {
            rows: std::array::from_fn(|row| {
                std::array::from_fn(|column| {
                    (0..3)
                        .map(|inner| next.rows[row][inner] * self.rows[inner][column])
                        .sum()
                })
            }),
        }
    }

    #[must_use]
    pub fn apply(&self, point: Point) -> Option<Point> {
        let [x, y, w] = self.homogeneous(point);
        (w.abs() > SINGULAR).then(|| Point::new((x / w) as f32, (y / w) as f32))
    }

    #[must_use]
    pub fn depth(&self, point: Point) -> f64 {
        self.homogeneous(point)[2]
    }

    #[must_use]
    pub fn inverse(&self) -> Option<Homography> {
        let m = self.rows;
        let cofactor = |row: usize, column: usize| {
            let (r0, r1) = ((row + 1) % 3, (row + 2) % 3);
            let (c0, c1) = ((column + 1) % 3, (column + 2) % 3);
            m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]
        };
        let determinant: f64 = (0..3)
            .map(|column| m[0][column] * cofactor(0, column))
            .sum();
        let scale = m
            .iter()
            .flatten()
            .fold(0.0f64, |most, value| most.max(value.abs()));
        if !determinant.is_finite() || determinant.abs() <= SINGULAR * scale.powi(3) {
            return None;
        }
        Some(Homography {
            rows: std::array::from_fn(|row| {
                std::array::from_fn(|column| cofactor(column, row) / determinant)
            }),
        })
    }

    fn homogeneous(&self, point: Point) -> [f64; 3] {
        let (x, y) = (f64::from(point.x), f64::from(point.y));
        self.rows.map(|row| row[0] * x + row[1] * y + row[2])
    }
}
