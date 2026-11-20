use rikka_physics_math::Vec3;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SegmentDistance {
    pub point1: Vec3,
    pub fraction1: f32,
    pub point2: Vec3,
    pub fraction2: f32,
}

impl SegmentDistance {
    #[inline]
    pub fn distance_squared(&self) -> f32 {
        self.point1.distance_squared(self.point2)
    }

    #[inline]
    pub(crate) fn is_within_segments(&self) -> bool {
        (0.0..=1.0).contains(&self.fraction1) && (0.0..=1.0).contains(&self.fraction2)
    }
}

pub fn segment_distance(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> SegmentDistance {
    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = d1.dot(d1);
    let b = d1.dot(d2);
    let c = d1.dot(r);
    let e = d2.dot(d2);
    let f = d2.dot(r);
    let degenerate = 100.0 * f32::EPSILON;

    if a < degenerate && e < degenerate {
        return SegmentDistance {
            point1: p1,
            fraction1: 0.0,
            point2: p2,
            fraction2: 0.0,
        };
    }
    if a < degenerate {
        let s2 = (f / e).clamp(0.0, 1.0);
        return SegmentDistance {
            point1: p1,
            fraction1: 0.0,
            point2: p2 + s2 * d2,
            fraction2: s2,
        };
    }
    if e < degenerate {
        let s1 = (-c / a).clamp(0.0, 1.0);
        return SegmentDistance {
            point1: p1 + s1 * d1,
            fraction1: s1,
            point2: p2,
            fraction2: 0.0,
        };
    }

    let denominator = a * e - b * b;
    let mut s1 = if denominator > 1000.0 * f32::MIN_POSITIVE {
        ((b * f - c * e) / denominator).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut s2 = (b * s1 + f) / e;
    if s2 < 0.0 {
        s1 = (-c / a).clamp(0.0, 1.0);
        s2 = 0.0;
    } else if s2 > 1.0 {
        s1 = ((b - c) / a).clamp(0.0, 1.0);
        s2 = 1.0;
    }

    SegmentDistance {
        point1: p1 + s1 * d1,
        fraction1: s1,
        point2: p2 + s2 * d2,
        fraction2: s2,
    }
}

pub(crate) fn line_distance(p1: Vec3, d1: Vec3, p2: Vec3, d2: Vec3) -> SegmentDistance {
    let a11 = d1.dot(d1);
    let a12 = -d1.dot(d2);
    let a21 = d2.dot(d1);
    let a22 = -d2.dot(d2);
    let w = p1 - p2;
    let b1 = -d1.dot(w);
    let b2 = -d2.dot(w);
    let determinant = a11 * a22 - a12 * a21;
    if determinant * determinant < 1000.0 * f32::MIN_POSITIVE {
        let s1 = (p2 - p1).dot(d1) / d1.dot(d1);
        return SegmentDistance {
            point1: p1 + s1 * d1,
            fraction1: s1,
            point2: p2,
            fraction2: 0.0,
        };
    }
    let s1 = (a22 * b1 - a12 * b2) / determinant;
    let s2 = (a11 * b2 - a21 * b1) / determinant;
    SegmentDistance {
        point1: p1 + s1 * d1,
        fraction1: s1,
        point2: p2 + s2 * d2,
        fraction2: s2,
    }
}

#[inline]
pub(crate) fn closest_on_segment(a: Vec3, b: Vec3, q: Vec3) -> Vec3 {
    let ab = b - a;
    let alpha = ab.dot(q - a);
    if alpha <= 0.0 {
        return a;
    }
    let denominator = ab.dot(ab);
    if alpha > denominator {
        return b;
    }
    a + (alpha / denominator) * ab
}
