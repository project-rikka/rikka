use rikka_physics_math::{F32x4, Float, MaskW, Vec3, Vec3W, Vector};

use super::hull_view::HullView;
use crate::convex::{argmax, horizontal_argmax, rows, soa_stride};

const WIDTH: usize = 4;
const PARALLEL_TOLERANCE: f32 = 0.005 * 0.005;

type Lanes = Vec3W<F32x4>;

#[derive(Clone, Copy, Debug)]
pub(crate) struct EdgeGeometry {
    pub(crate) point: Vec3,
    delta: Vec3,
    normal1: Vec3,
    normal2: Vec3,
}

impl EdgeGeometry {
    const PADDING: Self = Self {
        point: Vec3::ZERO,
        delta: Vec3::ZERO,
        normal1: Vec3::ZERO,
        normal2: Vec3::ZERO,
    };

    #[inline]
    pub(crate) fn from_hull(hull: &HullView<'_>, index: usize) -> Self {
        let edge = hull.edges[index];
        let twin = hull.edges[edge.twin as usize];
        let point = hull.points[edge.origin as usize];
        Self {
            point,
            delta: hull.points[twin.origin as usize] - point,
            normal1: hull.planes[edge.face as usize].normal,
            normal2: hull.planes[twin.face as usize].normal,
        }
    }

    #[inline]
    pub(crate) fn axis(&self, triangle_normal: Vec3, triangle_edge: Vec3) -> Option<Vec3> {
        let cab = self.normal1.dot(triangle_edge);
        let dab = self.normal2.dot(triangle_edge);
        let bcd = self.delta.dot(triangle_normal);
        if cab * dab >= 0.0 || cab * bcd <= 0.0 {
            return None;
        }
        if (cab * cab).max(dab * dab) < parallel_tolerance(triangle_edge) {
            return None;
        }
        let axis = self.normal1 + (self.normal2 - self.normal1) * (cab / (cab - dab));
        let length = axis.length();
        Some(if length > f32::EPSILON {
            axis * (1.0 / length)
        } else {
            Vec3::ZERO
        })
    }
}

#[inline]
fn parallel_tolerance(edge: Vec3) -> f32 {
    PARALLEL_TOLERANCE * edge.length_squared()
}

#[derive(Clone, Copy, Debug, Default)]
struct PlaneBlock {
    normal: Lanes,
    offset: F32x4,
}

#[derive(Clone, Copy, Debug, Default)]
struct EdgeBlock {
    point: Lanes,
    delta: Lanes,
    normal1: Lanes,
    normal2: Lanes,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FaceQuery {
    pub(crate) face: usize,
    pub(crate) vertex: usize,
    pub(crate) separation: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EdgeQuery {
    pub(crate) hull_edge: usize,
    pub(crate) triangle_edge: usize,
    pub(crate) axis: Vec3,
    pub(crate) separation: f32,
}

#[derive(Debug, Default)]
pub(crate) struct HullTable {
    points: Vec<Lanes>,
    point_count: usize,
    planes: Vec<PlaneBlock>,
    plane_count: usize,
    edges: Vec<EdgeBlock>,
    edge_count: usize,
    filled: u8,
}

const POINTS: u8 = 1;
const PLANES: u8 = 2;
const EDGES: u8 = 4;

#[inline(always)]
fn load4(values: &[f32], start: usize) -> F32x4 {
    let window = &values[start..start + WIDTH];
    F32x4::from_array([window[0], window[1], window[2], window[3]])
}

#[inline]
fn blocks(count: usize) -> impl Iterator<Item = usize> {
    (0..count).step_by(WIDTH)
}

#[inline]
fn lanes_of(count: usize, start: usize) -> usize {
    (count - start).min(WIDTH)
}

impl HullTable {
    #[inline]
    pub(crate) fn invalidate(&mut self) {
        self.filled = 0;
    }

    #[inline]
    fn prepare(&mut self, hull: &HullView<'_>, part: u8) {
        if self.filled & part == 0 {
            match part {
                POINTS => self.fill_points(hull),
                PLANES => self.fill_planes(hull),
                _ => self.fill_edges(hull),
            }
            self.filled |= part;
        }
    }

    fn fill_points(&mut self, hull: &HullView<'_>) {
        self.point_count = hull.points.len();
        self.points.clear();
        self.points.extend(blocks(self.point_count).map(|start| {
            Lanes::from_fn(|lane| {
                hull.points
                    .get(start + lane)
                    .copied()
                    .unwrap_or(hull.points[0])
            })
        }));
    }

    fn fill_planes(&mut self, hull: &HullView<'_>) {
        self.plane_count = hull.planes.len();
        self.planes.clear();
        if let Some(soa) = hull.soa_normals {
            let [nx, ny, nz] = rows::<3>(soa, soa_stride(self.plane_count));
            self.planes.extend(blocks(self.plane_count).map(|start| {
                let offset = F32x4::from_fn(|lane| {
                    hull.planes
                        .get(start + lane)
                        .map_or(0.0, |plane| plane.offset)
                });
                PlaneBlock {
                    normal: Lanes::new(load4(nx, start), load4(ny, start), load4(nz, start)),
                    offset,
                }
            }));
            return;
        }
        self.planes.extend(blocks(self.plane_count).map(|start| {
            let planes: [_; WIDTH] = std::array::from_fn(|lane| {
                hull.planes.get(start + lane).copied().unwrap_or_default()
            });
            PlaneBlock {
                normal: Lanes::from_fn(|lane| planes[lane].normal),
                offset: F32x4::from_fn(|lane| planes[lane].offset),
            }
        }));
    }

    fn fill_edges(&mut self, hull: &HullView<'_>) {
        self.edge_count = hull.edges.len() / 2;
        self.edges.clear();
        let count = self.edge_count;
        if let Some(edge_normals) = hull.edge_normals {
            let edge_rows = rows::<6>(edge_normals, soa_stride(count));
            let row = |index: usize, start: usize| load4(edge_rows[index], start);
            self.edges.extend(blocks(count).map(|start| {
                let mut points = [Vec3::ZERO; WIDTH];
                let mut deltas = [Vec3::ZERO; WIDTH];
                for lane in 0..lanes_of(count, start) {
                    let edge = hull.edges[2 * (start + lane)];
                    let twin = hull.edges[edge.twin as usize];
                    let point = hull.points[edge.origin as usize];
                    points[lane] = point;
                    deltas[lane] = hull.points[twin.origin as usize] - point;
                }
                EdgeBlock {
                    point: Lanes::from_fn(|lane| points[lane]),
                    delta: Lanes::from_fn(|lane| deltas[lane]),
                    normal1: Lanes::new(row(0, start), row(1, start), row(2, start)),
                    normal2: Lanes::new(row(3, start), row(4, start), row(5, start)),
                }
            }));
            return;
        }
        self.edges.extend(blocks(count).map(|start| {
            let edges: [EdgeGeometry; WIDTH] = std::array::from_fn(|lane| {
                if start + lane < count {
                    EdgeGeometry::from_hull(hull, 2 * (start + lane))
                } else {
                    EdgeGeometry::PADDING
                }
            });
            EdgeBlock {
                point: Lanes::from_fn(|lane| edges[lane].point),
                delta: Lanes::from_fn(|lane| edges[lane].delta),
                normal1: Lanes::from_fn(|lane| edges[lane].normal1),
                normal2: Lanes::from_fn(|lane| edges[lane].normal2),
            }
        }));
    }

    pub(crate) fn support(&mut self, hull: &HullView<'_>, direction: Vec3) -> usize {
        if let Some(soa) = hull.soa_points {
            let vertices = rows::<3>(soa, soa_stride(hull.points.len()));
            return argmax(vertices, direction).min(hull.points.len() - 1);
        }
        self.prepare(hull, POINTS);
        let direction = Lanes::splat(direction);
        let step = F32x4::splat(WIDTH as f32);
        let mut index = F32x4::from_array([0.0, 1.0, 2.0, 3.0]);
        let mut best_index = index;
        let mut best_value = self.points[0].dot(direction);
        for block in &self.points[1..] {
            index += step;
            let values = block.dot(direction);
            let better = values.simd_gt(best_value);
            best_value = F32x4::select(better, values, best_value);
            best_index = F32x4::select(better, index, best_index);
        }
        horizontal_argmax(best_value, best_index)
    }

    pub(crate) fn deepest_face(&mut self, hull: &HullView<'_>, points: &[Vec3; 3]) -> FaceQuery {
        self.prepare(hull, PLANES);
        let [p0, p1, p2] = points.map(Lanes::splat);
        let [one, two] = [F32x4::splat(1.0), F32x4::splat(2.0)];
        let mut best = FaceQuery {
            face: 0,
            vertex: 0,
            separation: f32::NEG_INFINITY,
        };
        for (block, start) in self.planes.iter().zip(blocks(self.plane_count)) {
            let s0 = block.normal.dot(p0);
            let s1 = block.normal.dot(p1);
            let s2 = block.normal.dot(p2);
            let first = s1.simd_lt(s0);
            let lowest = F32x4::select(first, s1, s0);
            let vertex = F32x4::select(first, one, F32x4::default());
            let second = s2.simd_lt(lowest);
            let lowest = F32x4::select(second, s2, lowest);
            let vertex = F32x4::select(second, two, vertex).to_array::<4>();
            let separations = (lowest - block.offset).to_array::<4>();
            for lane in 0..lanes_of(self.plane_count, start) {
                if separations[lane] > best.separation {
                    best = FaceQuery {
                        face: start + lane,
                        vertex: vertex[lane] as usize,
                        separation: separations[lane],
                    };
                }
            }
        }
        best
    }

    pub(crate) fn best_edge_pair(
        &mut self,
        hull: &HullView<'_>,
        points: &[Vec3; 3],
        edges: &[Vec3; 3],
        normal: Vec3,
    ) -> Option<EdgeQuery> {
        self.prepare(hull, EDGES);
        let zero = F32x4::default();
        let normal = Lanes::splat(normal);
        let tolerances = edges.map(|edge| F32x4::splat(parallel_tolerance(edge)));
        let splat_edges = edges.map(Lanes::splat);
        let splat_points = points.map(Lanes::splat);
        let mut best: Option<EdgeQuery> = None;
        for (block, start) in self.edges.iter().zip(blocks(self.edge_count)) {
            let bcd = block.delta.dot(normal);
            let mut axes = [Lanes::zero(); 3];
            let mut separations = [[0.0; WIDTH]; 3];
            let mut valid = [0u32; 3];
            for index in 0..3 {
                let edge = splat_edges[index];
                let cab = block.normal1.dot(edge);
                let dab = block.normal2.dot(edge);
                let crossing = !(cab * dab).simd_ge(zero) & !(cab * bcd).simd_le(zero);
                let steep = !(cab * cab).max(dab * dab).simd_lt(tolerances[index]);
                valid[index] = (crossing & steep).bits();
                if valid[index] == 0 {
                    continue;
                }
                let axis = block.normal1 + (block.normal2 - block.normal1) * (cab / (cab - dab));
                axes[index] = axis.normalize_or_zero();
                separations[index] = axes[index]
                    .dot(splat_points[index] - block.point)
                    .to_array();
            }
            if valid == [0; 3] {
                continue;
            }
            let count = lanes_of(self.edge_count, start);
            for (lane, hull_edge) in (start..start + count).enumerate() {
                for index in 0..3 {
                    let separation = separations[index][lane];
                    if valid[index] & (1 << lane) != 0
                        && best.is_none_or(|best| separation > best.separation)
                    {
                        best = Some(EdgeQuery {
                            hull_edge: 2 * hull_edge,
                            triangle_edge: index,
                            axis: axes[index].lane(lane),
                            separation,
                        });
                    }
                }
            }
        }
        best
    }
}
