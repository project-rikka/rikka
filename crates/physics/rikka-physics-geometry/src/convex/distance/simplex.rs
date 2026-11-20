use rikka_physics_math::Vec3;

use crate::convex::support::SupportPoint;

pub(crate) const MAX_SIMPLEX_VERTICES: usize = 4;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SimplexVertex {
    pub(crate) point_a: Vec3,
    pub(crate) local_b: Vec3,
    pub(crate) w: Vec3,
    pub(crate) weight: f32,
    pub(crate) id_a: u32,
    pub(crate) id_b: u32,
    pub(crate) heading_a: u32,
    pub(crate) heading_b: u32,
}

impl SimplexVertex {
    #[inline]
    pub(crate) fn new(
        a: SupportPoint,
        local_b: SupportPoint,
        point_b: Vec3,
        [heading_a, heading_b]: [u32; 2],
    ) -> Self {
        Self {
            point_a: a.point,
            local_b: local_b.point,
            w: point_b - a.point,
            weight: 0.0,
            id_a: a.id,
            id_b: local_b.id,
            heading_a,
            heading_b,
        }
    }

    #[inline]
    pub(crate) fn point_b(&self) -> Vec3 {
        self.point_a + self.w
    }

    #[inline]
    pub(crate) fn support_a(&self) -> SupportPoint {
        SupportPoint::new(self.point_a, self.id_a)
    }

    #[inline]
    pub(crate) fn support_b(&self) -> SupportPoint {
        SupportPoint::new(self.local_b, self.id_b)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Simplex {
    pub(crate) vertices: [SimplexVertex; MAX_SIMPLEX_VERTICES],
    pub(crate) count: usize,
}

fn edge_coordinates(a: Vec3, b: Vec3) -> [f32; 3] {
    let ab = b - a;
    [b.dot(ab), -a.dot(ab), ab.dot(ab)]
}

fn triangle_coordinates(a: Vec3, b: Vec3, c: Vec3) -> [f32; 4] {
    let n = (b - a).cross(c - a);
    [
        b.cross(c).dot(n),
        c.cross(a).dot(n),
        a.cross(b).dot(n),
        n.dot(n),
    ]
}

fn triple(a: Vec3, b: Vec3, c: Vec3) -> f32 {
    a.dot(b.cross(c))
}

fn tetrahedron_coordinates(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> [f32; 5] {
    let divisor = triple(b - a, c - a, d - a);
    let sign = if divisor < 0.0 { -1.0 } else { 1.0 };
    [
        sign * triple(b, c, d),
        sign * triple(a, d, c),
        sign * triple(a, b, d),
        sign * triple(a, c, b),
        sign * divisor,
    ]
}

impl Simplex {
    pub(crate) fn metric(&self) -> f32 {
        let v = &self.vertices;
        match self.count {
            2 => v[0].w.distance(v[1].w),
            3 => 0.5 * (v[1].w - v[0].w).cross(v[2].w - v[0].w).length(),
            4 => triple(v[1].w - v[0].w, v[2].w - v[0].w, v[3].w - v[0].w) / 6.0,
            _ => 0.0,
        }
    }

    pub(crate) fn closest_point(&self) -> Vec3 {
        self.blend(|vertex| vertex.w)
    }

    pub(crate) fn witness_points(&self) -> (Vec3, Vec3) {
        if self.count == MAX_SIMPLEX_VERTICES {
            let point = self.blend(|vertex| vertex.point_a);
            return (point, point);
        }
        (
            self.blend(|vertex| vertex.point_a),
            self.blend(SimplexVertex::point_b),
        )
    }

    fn blend(&self, value: impl Fn(&SimplexVertex) -> Vec3) -> Vec3 {
        let v = &self.vertices;
        match self.count {
            1 => value(&v[0]),
            2 => v[0].weight * value(&v[0]) + v[1].weight * value(&v[1]),
            3 => {
                v[0].weight * value(&v[0]) + v[1].weight * value(&v[1]) + v[2].weight * value(&v[2])
            }
            _ => {
                (v[0].weight * value(&v[0]) + v[1].weight * value(&v[1]))
                    + (v[2].weight * value(&v[2]) + v[3].weight * value(&v[3]))
            }
        }
    }

    pub(crate) fn search_direction(&self) -> Vec3 {
        let v = &self.vertices;
        match self.count {
            1 => -v[0].w,
            2 => {
                let ab = v[1].w - v[0].w;
                ab.cross(-v[0].w).cross(ab)
            }
            _ => {
                let n = (v[1].w - v[0].w).cross(v[2].w - v[0].w);
                if n.dot(v[0].w) < 0.0 { n } else { -n }
            }
        }
    }

    pub(crate) fn solve(&mut self) -> bool {
        match self.count {
            1 => {
                self.vertices[0].weight = 1.0;
                true
            }
            2 => self.solve2(),
            3 => self.solve3(),
            _ => self.solve4(),
        }
    }

    fn keep1(&mut self, vertex: SimplexVertex) -> bool {
        self.count = 1;
        self.vertices[0] = vertex;
        self.vertices[0].weight = 1.0;
        true
    }

    fn keep2(&mut self, a: SimplexVertex, b: SimplexVertex, w: [f32; 3]) -> bool {
        self.count = 2;
        self.vertices[0] = a;
        self.vertices[1] = b;
        if w[2] <= 0.0 {
            return false;
        }
        self.vertices[0].weight = w[0] / w[2];
        self.vertices[1].weight = w[1] / w[2];
        true
    }

    fn keep3(&mut self, a: SimplexVertex, b: SimplexVertex, c: SimplexVertex, w: [f32; 4]) -> bool {
        self.count = 3;
        self.vertices[0] = a;
        self.vertices[1] = b;
        self.vertices[2] = c;
        if w[3] <= 0.0 {
            return false;
        }
        self.vertices[0].weight = w[0] / w[3];
        self.vertices[1].weight = w[1] / w[3];
        self.vertices[2].weight = w[2] / w[3];
        true
    }

    fn solve2(&mut self) -> bool {
        let [a, b, ..] = self.vertices;
        let w = edge_coordinates(a.w, b.w);
        if w[1] <= 0.0 {
            return self.keep1(a);
        }
        if w[0] <= 0.0 {
            return self.keep1(b);
        }
        self.keep2(a, b, w)
    }

    fn solve3(&mut self) -> bool {
        let [a, b, c, _] = self.vertices;
        let ab = edge_coordinates(a.w, b.w);
        let bc = edge_coordinates(b.w, c.w);
        let ca = edge_coordinates(c.w, a.w);
        if ab[1] <= 0.0 && ca[0] <= 0.0 {
            return self.keep1(a);
        }
        if bc[1] <= 0.0 && ab[0] <= 0.0 {
            return self.keep1(b);
        }
        if ca[1] <= 0.0 && bc[0] <= 0.0 {
            return self.keep1(c);
        }
        let abc = triangle_coordinates(a.w, b.w, c.w);
        if abc[2] <= 0.0 && ab[0] > 0.0 && ab[1] > 0.0 {
            return self.keep2(a, b, ab);
        }
        if abc[0] <= 0.0 && bc[0] > 0.0 && bc[1] > 0.0 {
            return self.keep2(b, c, bc);
        }
        if abc[1] <= 0.0 && ca[0] > 0.0 && ca[1] > 0.0 {
            return self.keep2(c, a, ca);
        }
        self.keep3(a, b, c, abc)
    }

    fn solve4(&mut self) -> bool {
        let [a, b, c, d] = self.vertices;
        let ab = edge_coordinates(a.w, b.w);
        let ac = edge_coordinates(a.w, c.w);
        let ad = edge_coordinates(a.w, d.w);
        let bc = edge_coordinates(b.w, c.w);
        let cd = edge_coordinates(c.w, d.w);
        let db = edge_coordinates(d.w, b.w);

        if ab[1] <= 0.0 && ac[1] <= 0.0 && ad[1] <= 0.0 {
            return self.keep1(a);
        }
        if ab[0] <= 0.0 && db[0] <= 0.0 && bc[1] <= 0.0 {
            return self.keep1(b);
        }
        if ac[0] <= 0.0 && bc[0] <= 0.0 && cd[1] <= 0.0 {
            return self.keep1(c);
        }
        if ad[0] <= 0.0 && cd[0] <= 0.0 && db[1] <= 0.0 {
            return self.keep1(d);
        }

        let acb = triangle_coordinates(a.w, c.w, b.w);
        let abd = triangle_coordinates(a.w, b.w, d.w);
        let adc = triangle_coordinates(a.w, d.w, c.w);
        let bcd = triangle_coordinates(b.w, c.w, d.w);

        if abd[2] <= 0.0 && acb[1] <= 0.0 && ab[0] > 0.0 && ab[1] > 0.0 {
            return self.keep2(a, b, ab);
        }
        if acb[2] <= 0.0 && adc[1] <= 0.0 && ac[0] > 0.0 && ac[1] > 0.0 {
            return self.keep2(a, c, ac);
        }
        if adc[2] <= 0.0 && abd[1] <= 0.0 && ad[0] > 0.0 && ad[1] > 0.0 {
            return self.keep2(a, d, ad);
        }
        if acb[0] <= 0.0 && bcd[2] <= 0.0 && bc[0] > 0.0 && bc[1] > 0.0 {
            return self.keep2(b, c, bc);
        }
        if adc[0] <= 0.0 && bcd[0] <= 0.0 && cd[0] > 0.0 && cd[1] > 0.0 {
            return self.keep2(c, d, cd);
        }
        if abd[0] <= 0.0 && bcd[1] <= 0.0 && db[0] > 0.0 && db[1] > 0.0 {
            return self.keep2(d, b, db);
        }

        let abcd = tetrahedron_coordinates(a.w, b.w, c.w, d.w);
        let positive = |w: [f32; 4]| w[0] > 0.0 && w[1] > 0.0 && w[2] > 0.0;

        if abcd[3] < 0.0 && positive(acb) {
            return self.keep3(a, c, b, acb);
        }
        if abcd[2] < 0.0 && positive(abd) {
            return self.keep3(a, b, d, abd);
        }
        if abcd[1] < 0.0 && positive(adc) {
            return self.keep3(a, d, c, adc);
        }
        if abcd[0] < 0.0 && positive(bcd) {
            return self.keep3(b, c, d, bcd);
        }

        if abcd[4] <= 0.0 {
            return false;
        }
        for (vertex, weight) in self.vertices.iter_mut().zip(abcd) {
            vertex.weight = weight / abcd[4];
        }
        true
    }
}
