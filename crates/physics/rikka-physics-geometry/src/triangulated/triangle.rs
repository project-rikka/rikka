use rikka_physics_math::{Aabb, F32x4, Float, M32x4, MaskW, Vec3, Vec3W, Vector};

type Mask4 = M32x4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct EdgeFlags(pub u8);

impl EdgeFlags {
    pub const NONE: Self = Self(0);
    const CONCAVE: u8 = 0x01;
    const INVERSE_CONCAVE: u8 = 0x10;

    #[inline]
    pub fn concave(edge: usize) -> Self {
        Self(Self::CONCAVE << edge)
    }

    #[inline]
    pub fn inverse_concave(edge: usize) -> Self {
        Self(Self::INVERSE_CONCAVE << edge)
    }

    #[inline]
    pub fn flat(edge: usize) -> Self {
        Self((Self::CONCAVE | Self::INVERSE_CONCAVE) << edge)
    }

    #[inline]
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[inline]
    pub fn is_concave(self, edge: usize) -> bool {
        self.contains(Self::concave(edge))
    }

    #[inline]
    pub fn is_flat(self, edge: usize) -> bool {
        self.contains(Self::flat(edge))
    }

    #[inline]
    pub fn all_flat(self) -> bool {
        (0..3).all(|edge| self.is_flat(edge))
    }

    #[inline]
    pub fn inverted(self) -> Self {
        Self(((self.0 >> 4) & 0x07) | ((self.0 & 0x07) << 4))
    }

    #[inline]
    pub fn mirrored(self) -> Self {
        let edge = |bits: u8, from: usize, to: usize| {
            let concave = (bits >> from) & Self::CONCAVE;
            let inverse = (bits >> (from + 4)) & Self::CONCAVE;
            (concave << to) | (inverse << (to + 4))
        };
        Self(edge(self.0, 0, 2) | edge(self.0, 1, 1) | edge(self.0, 2, 0))
    }
}

impl std::ops::BitOr for EdgeFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl std::ops::BitOrAssign for EdgeFlags {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Triangle {
    pub vertices: [Vec3; 3],
    pub indices: [u32; 3],
    pub flags: EdgeFlags,
    pub material: u8,
    pub index: u32,
}

impl Triangle {
    #[inline]
    pub fn normal(&self) -> Vec3 {
        let [a, b, c] = self.vertices;
        (b - a).cross(c - a).normalize_or_zero()
    }

    #[inline]
    pub fn bounds(&self) -> Aabb {
        let [a, b, c] = self.vertices;
        Aabb::new(a.min(b).min(c), a.max(b).max(c))
    }

    #[inline]
    pub(crate) fn transformed(mut self, map: impl Fn(Vec3) -> Vec3) -> Self {
        self.vertices = map3(self.vertices, map);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TriangleFeature {
    Vertex(u8),
    Edge(u8),
    Face,
}

#[inline]
pub(crate) fn signed_volume(a: Vec3, b: Vec3, c: Vec3, p: Vec3) -> f32 {
    (b - a).cross(c - a).dot(p - a)
}

pub(crate) fn closest_point(a: Vec3, b: Vec3, c: Vec3, q: Vec3) -> (Vec3, TriangleFeature) {
    let ab = b - a;
    let ac = c - a;
    let aq = q - a;
    let d1 = ab.dot(aq);
    let d2 = ac.dot(aq);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a, TriangleFeature::Vertex(0));
    }
    let bq = q - b;
    let d3 = ab.dot(bq);
    let d4 = ac.dot(bq);
    if d3 > 0.0 && d4 <= d3 {
        return (b, TriangleFeature::Vertex(1));
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let t = d1 / (d1 - d3);
        return (a + t * ab, TriangleFeature::Edge(0));
    }
    let cq = q - c;
    let d5 = ab.dot(cq);
    let d6 = ac.dot(cq);
    if d6 >= 0.0 && d5 <= d6 {
        return (c, TriangleFeature::Vertex(2));
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let t = d2 / (d2 - d6);
        return (a + t * ac, TriangleFeature::Edge(2));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 >= d3 && d5 >= d6 {
        let t = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (b + t * (c - b), TriangleFeature::Edge(1));
    }
    let denominator = va + vb + vc;
    let t1 = vb / denominator;
    let t2 = vc / denominator;
    (a + t1 * ab + t2 * ac, TriangleFeature::Face)
}

pub(crate) fn ray_triangle(
    origin: Vec3,
    translation: Vec3,
    [a, b, c]: [Vec3; 3],
    max_fraction: f32,
) -> Option<f32> {
    let outside = |p: Vec3, q: Vec3| (q - p).cross(0.5 * (p + q) - origin).dot(translation) < 0.0;
    if outside(b, c) || outside(c, a) || outside(a, b) {
        return None;
    }
    let normal = (b - a).cross(c - a);
    let denominator = normal.dot(translation);
    if denominator >= 0.0 {
        return None;
    }
    let fraction = normal.dot(a - origin) / denominator;
    (0.0..=max_fraction).contains(&fraction).then_some(fraction)
}

#[inline(always)]
pub(crate) fn map3<T: Copy, U>(values: [T; 3], f: impl Fn(T) -> U) -> [U; 3] {
    [f(values[0]), f(values[1]), f(values[2])]
}

#[inline]
fn modified_cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(
        a.y * b.z + a.z * b.y,
        a.z * b.x + a.x * b.z,
        a.x * b.y + a.y * b.x,
    )
}

#[inline]
fn edge_separates(edge: Vec3, sum: Vec3, other: Vec3, extent: Vec3) -> bool {
    let separation =
        edge.cross(sum).abs() - edge.cross(other).abs() - 2.0 * modified_cross(edge.abs(), extent);
    separation.cmpgt(Vec3::ZERO).any()
}

#[inline]
pub(crate) fn triangle_overlaps_box(center: Vec3, extent: Vec3, [a, b, c]: [Vec3; 3]) -> bool {
    let (a, b, c) = (a - center, b - center, c - center);
    let low = a.min(b).min(c);
    let high = a.max(b).max(c);
    if low.cmpgt(extent).any() || high.cmplt(-extent).any() {
        return false;
    }
    let (e1, e2, e3) = (b - a, c - b, a - c);
    let normal = e1.cross(e2);
    if normal.dot(a).abs() > normal.abs().dot(extent) {
        return false;
    }
    !(edge_separates(e1, a + c, e3, extent)
        || edge_separates(e2, a + b, e1, extent)
        || edge_separates(e3, b + c, e2, extent))
}

#[inline(always)]
fn abs4(v: Vec3W<F32x4>) -> Vec3W<F32x4> {
    Vec3W::new(v.x.abs(), v.y.abs(), v.z.abs())
}

#[inline(always)]
fn edges_separate4(
    edge: Vec3W<F32x4>,
    sum: Vec3W<F32x4>,
    other: Vec3W<F32x4>,
    extent: Vec3W<F32x4>,
) -> Mask4 {
    let reach = abs4(edge);
    let twice = F32x4::splat(2.0);
    let far = abs4(edge.cross(sum));
    let near = abs4(edge.cross(other));
    let x = far.x - near.x - twice * (reach.y * extent.z + reach.z * extent.y);
    let y = far.y - near.y - twice * (reach.z * extent.x + reach.x * extent.z);
    let z = far.z - near.z - twice * (reach.x * extent.y + reach.y * extent.x);
    let zero = F32x4::default();
    x.simd_gt(zero) | y.simd_gt(zero) | z.simd_gt(zero)
}

#[inline]
pub(crate) fn triangles_overlap_box(center: Vec3, extent: Vec3, triangles: &[[Vec3; 3]; 4]) -> u32 {
    let origin = Vec3W::<F32x4>::splat(center);
    let corner = |vertex: usize| Vec3W::from_fn(|lane| triangles[lane][vertex]) - origin;
    let (a, b, c) = (corner(0), corner(1), corner(2));
    let reach = Vec3W::<F32x4>::splat(extent);
    let outside = |p: F32x4, q: F32x4, r: F32x4, e: F32x4| {
        p.min(q).min(r).simd_gt(e) | p.max(q).max(r).simd_lt(-e)
    };
    let mut separated = outside(a.x, b.x, c.x, reach.x)
        | outside(a.y, b.y, c.y, reach.y)
        | outside(a.z, b.z, c.z, reach.z);
    let (e1, e2, e3) = (b - a, c - b, a - c);
    let normal = e1.cross(e2);
    let offset = (normal.x * a.x + normal.y * a.y + normal.z * a.z).abs();
    let radius = normal.x.abs() * reach.x + normal.y.abs() * reach.y + normal.z.abs() * reach.z;
    separated = separated
        | offset.simd_gt(radius)
        | edges_separate4(e1, a + c, e3, reach)
        | edges_separate4(e2, a + b, e1, reach)
        | edges_separate4(e3, b + c, e2, reach);
    !separated.bits() & 0b1111
}
