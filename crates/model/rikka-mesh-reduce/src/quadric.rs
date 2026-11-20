use glam::{DVec3, Vec3};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Quadric {
    a2: f64,
    ab: f64,
    ac: f64,
    ad: f64,
    b2: f64,
    bc: f64,
    bd: f64,
    c2: f64,
    cd: f64,
    d2: f64,
}

impl Quadric {
    pub(crate) fn from_plane(plane: [f64; 4]) -> Quadric {
        let [a, b, c, d] = plane;
        Quadric {
            a2: a * a,
            ab: a * b,
            ac: a * c,
            ad: a * d,
            b2: b * b,
            bc: b * c,
            bd: b * d,
            c2: c * c,
            cd: c * d,
            d2: d * d,
        }
    }

    pub(crate) fn add(&mut self, other: &Quadric) {
        self.a2 += other.a2;
        self.ab += other.ab;
        self.ac += other.ac;
        self.ad += other.ad;
        self.b2 += other.b2;
        self.bc += other.bc;
        self.bd += other.bd;
        self.c2 += other.c2;
        self.cd += other.cd;
        self.d2 += other.d2;
    }

    pub(crate) fn sum(a: &Quadric, b: &Quadric) -> Quadric {
        let mut sum = *a;
        sum.add(b);
        sum
    }

    pub(crate) fn scale(&mut self, factor: f64) {
        self.a2 *= factor;
        self.ab *= factor;
        self.ac *= factor;
        self.ad *= factor;
        self.b2 *= factor;
        self.bc *= factor;
        self.bd *= factor;
        self.c2 *= factor;
        self.cd *= factor;
        self.d2 *= factor;
    }

    pub(crate) fn evaluate(&self, point: DVec3) -> f64 {
        let (x, y, z) = (point.x, point.y, point.z);
        let xx = x * x;
        let xy = x * y;
        let xz = x * z;
        let yy = y * y;
        let yz = y * z;
        let zz = z * z;
        (self.a2 * xx)
            + (self.ab * 2.0 * xy)
            + (self.ac * 2.0 * xz)
            + (self.ad * 2.0 * x)
            + (self.b2 * yy)
            + (self.bc * 2.0 * yz)
            + (self.bd * 2.0 * y)
            + (self.c2 * zz)
            + (self.cd * 2.0 * z)
            + (self.d2)
    }

    pub(crate) fn optimize(&self, epsilon: f64) -> Option<DVec3> {
        let q = self;
        let det = q.a2 * (q.b2 * q.c2 - q.bc * q.bc) - q.ab * (q.ab * q.c2 - q.ac * q.bc)
            + q.ac * (q.ab * q.bc - q.ac * q.b2);
        if det.abs() <= epsilon {
            return None;
        }
        let inverse = 1.0 / det;
        let m = [
            [
                (q.b2 * q.c2 - q.bc * q.bc) * inverse,
                (q.ac * q.bc - q.ab * q.c2) * inverse,
                (q.ab * q.bc - q.ac * q.b2) * inverse,
            ],
            [
                (q.bc * q.ac - q.ab * q.c2) * inverse,
                (q.a2 * q.c2 - q.ac * q.ac) * inverse,
                (q.ac * q.ab - q.a2 * q.bc) * inverse,
            ],
            [
                (q.ab * q.bc - q.b2 * q.ac) * inverse,
                (q.ab * q.ac - q.a2 * q.bc) * inverse,
                (q.a2 * q.b2 - q.ab * q.ab) * inverse,
            ],
        ];
        let v = [q.ad, q.bd, q.cd];
        let result = DVec3::new(
            m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
            m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
            m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
        );
        Some(-result)
    }
}

pub(crate) fn plane_through(normal: DVec3, point: Vec3) -> [f64; 4] {
    let d = -(normal.x * point.x as f64 + normal.y * point.y as f64 + normal.z * point.z as f64);
    [normal.x, normal.y, normal.z, d]
}
