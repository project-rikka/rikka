use glam::{DVec3, Vec2, Vec3};

pub(crate) fn sign(value: f64) -> i32 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

pub(crate) fn ray_triangle_reference(
    origin: Vec3,
    direction: Vec3,
    triangle: [Vec3; 3],
) -> Option<f64> {
    let [a, b, c] = triangle.map(Vec3::as_dvec3);
    let (origin, direction) = (origin.as_dvec3(), direction.as_dvec3());
    let (edge1, edge2) = (b - a, c - a);
    let p = direction.cross(edge2);
    let determinant = edge1.dot(p);
    if determinant.abs() < 1e-14 {
        return None;
    }
    let inverse = 1.0 / determinant;
    let s = origin - a;
    let u = s.dot(p) * inverse;
    let q = s.cross(edge1);
    let v = direction.dot(q) * inverse;
    if u < 0.0 || v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = edge2.dot(q) * inverse;
    (t >= 0.0).then_some(t)
}

pub(crate) fn segment_distance_reference(point: DVec3, start: DVec3, end: DVec3) -> f64 {
    let direction = end - start;
    let length = direction.length_squared();
    let t = if length > 0.0 {
        ((point - start).dot(direction) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (start + direction * t).distance(point)
}

pub(crate) fn triangle_distance_reference(point: Vec3, triangle: [Vec3; 3]) -> f64 {
    let p = point.as_dvec3();
    let [a, b, c] = triangle.map(Vec3::as_dvec3);
    let edges = segment_distance_reference(p, a, b)
        .min(segment_distance_reference(p, b, c))
        .min(segment_distance_reference(p, c, a));
    let normal = (b - a).cross(c - a);
    if normal.length_squared() == 0.0 {
        return edges;
    }
    let normal = normal.normalize();
    let projected = p - normal * normal.dot(p - a);
    let inside = [(a, b), (b, c), (c, a)]
        .iter()
        .all(|&(start, end)| (end - start).cross(projected - start).dot(normal) >= 0.0);
    match inside {
        true => edges.min(normal.dot(p - a).abs()),
        false => edges,
    }
}

pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed ^ 0x9e37_79b9_7f4a_7c15)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    pub(crate) fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub(crate) fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }

    pub(crate) fn index(&mut self, count: usize) -> usize {
        (self.next_u64() % count as u64) as usize
    }

    pub(crate) fn vec3(&mut self, low: f32, high: f32) -> Vec3 {
        Vec3::new(
            self.range(low, high),
            self.range(low, high),
            self.range(low, high),
        )
    }

    pub(crate) fn vec2(&mut self, low: f32, high: f32) -> Vec2 {
        Vec2::new(self.range(low, high), self.range(low, high))
    }

    pub(crate) fn direction(&mut self) -> Vec3 {
        loop {
            let candidate = self.vec3(-1.0, 1.0);
            let length = candidate.length_squared();
            if length > 1e-4 && length <= 1.0 {
                return candidate / length.sqrt();
            }
        }
    }
}

pub(crate) fn triangle_soup(rng: &mut Rng, count: usize, extent: f32, size: f32) -> Vec<[Vec3; 3]> {
    (0..count)
        .map(|_| {
            let center = rng.vec3(-extent, extent);
            [
                center + rng.vec3(-size, size),
                center + rng.vec3(-size, size),
                center + rng.vec3(-size, size),
            ]
        })
        .collect()
}

pub(crate) struct Mesh {
    pub(crate) positions: Vec<Vec3>,
    pub(crate) triangles: Vec<[u32; 3]>,
}

impl Mesh {
    pub(crate) fn triangle(&self, index: usize) -> [Vec3; 3] {
        self.triangles[index].map(|vertex| self.positions[vertex as usize])
    }

    pub(crate) fn edges(&self) -> Vec<[u32; 2]> {
        let mut edges: Vec<[u32; 2]> = self
            .triangles
            .iter()
            .flat_map(|&[a, b, c]| [[a, b], [b, c], [c, a]])
            .map(|[a, b]| [a.min(b), a.max(b)])
            .collect();
        edges.sort_unstable();
        edges.dedup();
        edges
    }
}

pub(crate) fn grid_mesh(columns: u32, rows: u32, position: impl Fn(f32, f32) -> Vec3) -> Mesh {
    let mut positions = Vec::new();
    for row in 0..=rows {
        for column in 0..=columns {
            positions.push(position(
                column as f32 / columns as f32,
                row as f32 / rows as f32,
            ));
        }
    }
    let stride = columns + 1;
    let mut triangles = Vec::new();
    for row in 0..rows {
        for column in 0..columns {
            let corner = row * stride + column;
            triangles.push([corner, corner + 1, corner + stride + 1]);
            triangles.push([corner, corner + stride + 1, corner + stride]);
        }
    }
    Mesh {
        positions,
        triangles,
    }
}

pub(crate) fn uv_sphere(columns: u32, rows: u32, radius: f32) -> Mesh {
    let mut positions = vec![Vec3::Z * radius];
    for ring in 1..rows {
        let polar = ring as f32 / rows as f32 * std::f32::consts::PI;
        for column in 0..columns {
            let azimuth = column as f32 / columns as f32 * std::f32::consts::TAU;
            positions.push(
                Vec3::new(
                    polar.sin() * azimuth.cos(),
                    polar.sin() * azimuth.sin(),
                    polar.cos(),
                ) * radius,
            );
        }
    }
    positions.push(Vec3::NEG_Z * radius);
    let south = positions.len() as u32 - 1;
    let vertex = |ring: u32, column: u32| 1 + (ring - 1) * columns + column % columns;
    let mut triangles = Vec::new();
    for column in 0..columns {
        triangles.push([0, vertex(1, column), vertex(1, column + 1)]);
        for ring in 1..rows - 1 {
            let (a, b) = (vertex(ring, column), vertex(ring, column + 1));
            let (d, e) = (vertex(ring + 1, column), vertex(ring + 1, column + 1));
            triangles.push([a, d, e]);
            triangles.push([a, e, b]);
        }
        triangles.push([
            vertex(rows - 1, column),
            south,
            vertex(rows - 1, column + 1),
        ]);
    }
    Mesh {
        positions,
        triangles,
    }
}

pub(crate) fn torus(columns: u32, rows: u32, major: f32, minor: f32) -> Mesh {
    let mut positions = Vec::new();
    for row in 0..rows {
        let tube = row as f32 / rows as f32 * std::f32::consts::TAU;
        for column in 0..columns {
            let around = column as f32 / columns as f32 * std::f32::consts::TAU;
            let ring = major + minor * tube.cos();
            positions.push(Vec3::new(
                ring * around.cos(),
                ring * around.sin(),
                minor * tube.sin(),
            ));
        }
    }
    let vertex = |row: u32, column: u32| (row % rows) * columns + column % columns;
    let mut triangles = Vec::new();
    for row in 0..rows {
        for column in 0..columns {
            let (a, b) = (vertex(row, column), vertex(row, column + 1));
            let (c, d) = (vertex(row + 1, column + 1), vertex(row + 1, column));
            triangles.push([a, b, c]);
            triangles.push([a, c, d]);
        }
    }
    Mesh {
        positions,
        triangles,
    }
}
