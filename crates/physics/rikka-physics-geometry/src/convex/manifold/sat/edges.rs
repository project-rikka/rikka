use rikka_physics_math::{F32x4, Float, Mask, MaskW, Mat3, Vec3, Vector, Wide};

use super::{AxisKind, LANES, SeparatingAxis, row, valid_lanes};
use crate::convex::hull::Polyhedron;

const PARALLEL_EDGE_TOLERANCE: f32 = 0.005;
const SQUARED_EDGE_TOLERANCE: f32 = PARALLEL_EDGE_TOLERANCE * PARALLEL_EDGE_TOLERANCE;
const PARALLEL_TOLERANCE: f32 = 1.0e-4;
const CROSSING: f32 = -0.0001;
const MAX_EDGE_GROUPS: usize = 192;
const CHUNK_GROUPS: usize = 8;

type Lanes3 = [[f32; LANES]; 3];

#[inline(always)]
fn dot(a: [F32x4; 3], b: [F32x4; 3]) -> F32x4 {
    a[0] * b[0] + (a[1] * b[1] + a[2] * b[2])
}

#[inline(always)]
fn splat(v: Vec3) -> [F32x4; 3] {
    [F32x4::splat(v.x), F32x4::splat(v.y), F32x4::splat(v.z)]
}

#[inline(always)]
fn load(v: &Lanes3) -> [F32x4; 3] {
    v.map(F32x4::from_array)
}

#[derive(Clone, Copy)]
struct Edge {
    u: Vec3,
    v: Vec3,
    e: Vec3,
    o: Vec3,
}

impl Edge {
    #[inline(always)]
    fn from_pair(polyhedron: &Polyhedron<'_>, pair: usize) -> Self {
        let edge = polyhedron.edges[2 * pair];
        let twin = polyhedron.edges[2 * pair + 1];
        let o = polyhedron.points[edge.origin as usize];
        Self {
            u: polyhedron.planes[edge.face as usize].normal,
            v: polyhedron.planes[twin.face as usize].normal,
            e: polyhedron.points[twin.origin as usize] - o,
            o,
        }
    }

    #[inline(always)]
    fn moved(self, rotation: Mat3, translation: Vec3) -> Self {
        Self {
            u: rotation * self.u,
            v: rotation * self.v,
            e: rotation * self.e,
            o: rotation * self.o + translation,
        }
    }
}

#[derive(Clone, Copy)]
struct Arcs {
    length: f32,
    floor: f32,
}

impl Arcs {
    #[inline]
    fn masks(
        &self,
        polyhedron: &Polyhedron<'_>,
        toward: Vec3,
        other_inner: f32,
        out: &mut [u8; MAX_EDGE_GROUPS],
    ) -> u32 {
        let rows = polyhedron.edge_rows();
        let pairs = polyhedron.edge_pair_count();
        let [dx, dy, dz] = splat(toward);
        let floor = F32x4::splat(self.floor + other_inner);
        let length = F32x4::splat(self.length);
        let mut count = 0;
        for (group, slot) in out.iter_mut().enumerate().take(pairs.div_ceil(LANES)) {
            let a = row(rows[0], group) * dx + row(rows[1], group) * dy + row(rows[2], group) * dz;
            let b = row(rows[3], group) * dx + row(rows[4], group) * dy + row(rows[5], group) * dz;
            let c = row(rows[6], group);
            let bound = floor + row(rows[7], group);
            let s = F32x4::one() - c * c;
            let t = a * a + b * b - F32x4::splat(2.0) * a * b * c;
            let endpoint = a.max(b).simd_ge(bound);
            let interior = a.simd_ge(c * b)
                & b.simd_ge(c * a)
                & length.simd_ge(bound)
                & (bound.simd_le(F32x4::default())
                    | s.simd_lt(F32x4::splat(PARALLEL_TOLERANCE))
                    | t.simd_ge(bound * bound * s));
            let bits = (endpoint | interior).bits() & valid_lanes(pairs - group * LANES);
            *slot = bits as u8;
            count += bits.count_ones();
        }
        count
    }
}

#[derive(Clone, Copy, Default)]
struct EdgeGroup {
    u: Lanes3,
    v: Lanes3,
    e: Lanes3,
    o: Lanes3,
    tolerance: [f32; LANES],
    edge: [u16; LANES],
}

#[derive(Clone, Copy, Default)]
struct EdgeChunk {
    groups: [EdgeGroup; CHUNK_GROUPS],
    len: usize,
}

impl EdgeChunk {
    #[inline]
    fn push(&mut self, edge: Edge, pair: usize) {
        let group = &mut self.groups[self.len / LANES];
        let lane = self.len % LANES;
        for axis in 0..3 {
            group.u[axis][lane] = edge.u[axis];
            group.v[axis][lane] = edge.v[axis];
            group.e[axis][lane] = edge.e[axis];
            group.o[axis][lane] = edge.o[axis];
        }
        group.tolerance[lane] = SQUARED_EDGE_TOLERANCE * edge.e.length_squared();
        group.edge[lane] = (2 * pair) as u16;
        self.len += 1;
    }

    #[inline]
    fn is_full(&self) -> bool {
        self.len == CHUNK_GROUPS * LANES
    }
}

struct Pass<'p, 'a, const LANES_ARE_A: bool> {
    lanes: &'p Polyhedron<'a>,
    lane_masks: &'p [u8],
    scalars: &'p Polyhedron<'a>,
    scalar_masks: &'p [u8],
    rotation: Mat3,
    translation: Vec3,
    to_a: Mat3,
    separated_above: f32,
}

impl<const LANES_ARE_A: bool> Pass<'_, '_, LANES_ARE_A> {
    fn run(&self, best: &mut SeparatingAxis) -> bool {
        let mut chunk = EdgeChunk::default();
        for (group, &mask) in self.lane_masks.iter().enumerate() {
            let mut bits = u32::from(mask);
            while bits != 0 {
                let pair = group * LANES + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                chunk.push(Edge::from_pair(self.lanes, pair), pair);
                if chunk.is_full() {
                    if self.search(&chunk, best) {
                        return true;
                    }
                    chunk = EdgeChunk::default();
                }
            }
        }
        chunk.len > 0 && self.search(&chunk, best)
    }

    fn search(&self, chunk: &EdgeChunk, best: &mut SeparatingAxis) -> bool {
        let groups = &chunk.groups[..chunk.len.div_ceil(LANES)];
        let crossing = F32x4::splat(CROSSING);
        for (group_s, &mask) in self.scalar_masks.iter().enumerate() {
            let mut bits = u32::from(mask);
            while bits != 0 {
                let pair = group_s * LANES + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let scalar =
                    Edge::from_pair(self.scalars, pair).moved(self.rotation, self.translation);
                let [su, sv, se, so] = [scalar.u, scalar.v, scalar.e, scalar.o].map(splat);
                let scalar_tolerance =
                    F32x4::splat(SQUARED_EDGE_TOLERANCE * scalar.e.length_squared());

                for group in groups {
                    let [lu, lv, le] = [&group.u, &group.v, &group.e].map(load);
                    let (p, q, r, s) = if LANES_ARE_A {
                        (dot(su, le), dot(sv, le), dot(lu, se), dot(lv, se))
                    } else {
                        (dot(lu, se), dot(lv, se), dot(su, le), dot(sv, le))
                    };
                    let tolerance = if LANES_ARE_A {
                        F32x4::from_array(group.tolerance)
                    } else {
                        scalar_tolerance
                    };
                    let hit = (p * q).simd_lt(crossing)
                        & (r * s).simd_lt(crossing)
                        & (p * s).simd_lt(crossing)
                        & (p * p).max(q * q).simd_gt(tolerance);
                    if !hit.any() {
                        continue;
                    }

                    let t = p / (p - q);
                    let (u, v) = if LANES_ARE_A { (su, sv) } else { (lu, lv) };
                    let mut m = [0, 1, 2].map(|axis| u[axis] + t * (v[axis] - u[axis]));
                    let inverse_length = F32x4::one() / dot(m, m).sqrt();
                    m = m.map(|component| component * inverse_length);
                    let lo = load(&group.o);
                    let offset = if LANES_ARE_A {
                        [0, 1, 2].map(|axis| lo[axis] - so[axis])
                    } else {
                        [0, 1, 2].map(|axis| so[axis] - lo[axis])
                    };
                    let separation =
                        F32x4::select(hit, dot(m, offset), F32x4::splat(f32::NEG_INFINITY));
                    let mut improving = separation.simd_gt(F32x4::splat(best.separation)).bits();
                    if improving == 0 {
                        continue;
                    }
                    let values = separation.to_array::<4>();
                    let m = m.map(F32x4::to_array::<4>);
                    while improving != 0 {
                        let lane = improving.trailing_zeros() as usize;
                        improving &= improving - 1;
                        if values[lane] <= best.separation {
                            continue;
                        }
                        let normal = -Vec3::new(m[0][lane], m[1][lane], m[2][lane]);
                        let (edge_a, edge_b) = if LANES_ARE_A {
                            (group.edge[lane] as usize, 2 * pair)
                        } else {
                            (2 * pair, group.edge[lane] as usize)
                        };
                        *best = SeparatingAxis {
                            normal: if LANES_ARE_A {
                                normal
                            } else {
                                self.to_a * normal
                            },
                            separation: values[lane],
                            index_a: edge_a,
                            index_b: edge_b,
                            kind: AxisKind::EdgePair,
                        };
                        if values[lane] > self.separated_above {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct EdgeQuery {
    pub(super) rotation: Mat3,
    pub(super) translation: Vec3,
    pub(super) toward_b: Vec3,
    pub(super) toward_a: Vec3,
    pub(super) center_distance: f32,
    pub(super) floor: f32,
    pub(super) separated_above: f32,
}

impl EdgeQuery {
    pub(super) fn search(
        &self,
        a: &Polyhedron<'_>,
        b: &Polyhedron<'_>,
        best: &mut SeparatingAxis,
    ) -> bool {
        let arcs = Arcs {
            length: self.center_distance,
            floor: self.floor,
        };
        let mut masks_a = [0u8; MAX_EDGE_GROUPS];
        let count_a = arcs.masks(a, self.toward_b, b.inner_radius, &mut masks_a);
        if count_a == 0 {
            return false;
        }
        let mut masks_b = [0u8; MAX_EDGE_GROUPS];
        let count_b = arcs.masks(b, self.toward_a, a.inner_radius, &mut masks_b);
        if count_b == 0 {
            return false;
        }
        let masks_a = &masks_a[..a.edge_pair_count().div_ceil(LANES)];
        let masks_b = &masks_b[..b.edge_pair_count().div_ceil(LANES)];
        if count_a >= count_b {
            Pass::<true> {
                lanes: a,
                lane_masks: masks_a,
                scalars: b,
                scalar_masks: masks_b,
                rotation: self.rotation,
                translation: self.translation,
                to_a: Mat3::IDENTITY,
                separated_above: self.separated_above,
            }
            .run(best)
        } else {
            let inverse = self.rotation.transpose();
            Pass::<false> {
                lanes: b,
                lane_masks: masks_b,
                scalars: a,
                scalar_masks: masks_a,
                rotation: inverse,
                translation: -(inverse * self.translation),
                to_a: self.rotation,
                separated_above: self.separated_above,
            }
            .run(best)
        }
    }
}
