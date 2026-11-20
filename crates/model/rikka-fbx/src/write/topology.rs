use std::borrow::Cow;

use rikka_model_core::Mesh;
use rikka_model_core::glam::Vec3;
use rustc_hash::FxHashMap;

const SMOOTH_NORMAL_TOLERANCE: f32 = 1e-5;

pub(crate) struct ControlPoints {
    of_vertex: Vec<u32>,
    vertices: Vec<u32>,
}

impl ControlPoints {
    fn per_vertex(count: usize) -> Self {
        Self {
            of_vertex: (0..count as u32).collect(),
            vertices: (0..count as u32).collect(),
        }
    }

    fn compacted(vertex_points: &[u32]) -> Self {
        let mut points = vertex_points.to_vec();
        points.sort_unstable();
        points.dedup();
        let of_vertex: Vec<u32> = vertex_points
            .iter()
            .map(|point| points.partition_point(|used| used < point) as u32)
            .collect();
        let mut vertices = vec![u32::MAX; points.len()];
        for (vertex, &point) in of_vertex.iter().enumerate() {
            let representative = &mut vertices[point as usize];
            if *representative == u32::MAX {
                *representative = vertex as u32;
            }
        }
        Self {
            of_vertex,
            vertices,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.vertices.len()
    }

    pub(crate) fn of_vertex(&self, vertex: u32) -> u32 {
        self.of_vertex[vertex as usize]
    }

    pub(crate) fn vertex(&self, point: usize) -> u32 {
        self.vertices[point]
    }
}

pub(crate) struct Topology<'a> {
    pub points: ControlPoints,
    pub offsets: Cow<'a, [u32]>,
    pub corners: Cow<'a, [u32]>,
    pub submeshes: Cow<'a, [u32]>,
}

impl<'a> Topology<'a> {
    pub(crate) fn of(mesh: &'a Mesh) -> Self {
        match &mesh.polygons {
            Some(polygons) => Self {
                points: ControlPoints::compacted(&polygons.vertex_points),
                offsets: Cow::Borrowed(&polygons.offsets),
                corners: Cow::Borrowed(&polygons.corner_vertices),
                submeshes: Cow::Borrowed(&polygons.submeshes),
            },
            None => Self::triangles(mesh),
        }
    }

    fn triangles(mesh: &Mesh) -> Self {
        let mut corners = Vec::with_capacity(mesh.indices.len());
        let mut submeshes = Vec::with_capacity(mesh.indices.len() / 3);
        for (index, submesh) in mesh.submeshes.iter().enumerate() {
            let start = submesh.first_index as usize;
            let range = &mesh.indices[start..start + submesh.index_count as usize];
            for triangle in range.as_chunks::<3>().0 {
                corners.extend_from_slice(triangle);
                submeshes.push(index as u32);
            }
        }
        Self {
            points: ControlPoints::per_vertex(mesh.vertices.len()),
            offsets: Cow::Owned((0..=submeshes.len() as u32).map(|at| at * 3).collect()),
            corners: Cow::Owned(corners),
            submeshes: Cow::Owned(submeshes),
        }
    }

    fn sides(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.offsets.windows(2).flat_map(|pair| {
            let (start, end) = (pair[0] as usize, pair[1] as usize);
            (start..end).map(move |corner| {
                let next = match corner + 1 == end {
                    true => start,
                    false => corner + 1,
                };
                (corner, next)
            })
        })
    }

    pub(crate) fn point(&self, corner: usize) -> u32 {
        self.points.of_vertex(self.corners[corner])
    }

    pub(crate) fn polygon_vertex_index(&self) -> Vec<i32> {
        let mut indices: Vec<i32> = (0..self.corners.len())
            .map(|corner| self.point(corner) as i32)
            .collect();
        for &end in self.offsets.iter().skip(1) {
            let last = &mut indices[end as usize - 1];
            *last = !*last;
        }
        indices
    }

    pub(crate) fn edges(&self, normals: &[Vec3]) -> Edges {
        let mut edges = Edges::default();
        let mut found: FxHashMap<(u32, u32), usize> = FxHashMap::default();
        for (corner, next) in self.sides() {
            let (from, to) = (self.point(corner), self.point(next));
            let key = (from.min(to), from.max(to));
            let edge = *found.entry(key).or_insert_with(|| {
                edges.first_corners.push(corner as i32);
                edges.sides.push(Sides::One { corner, next });
                edges.sides.len() - 1
            });
            if edges.first_corners[edge] as usize == corner {
                continue;
            }
            edges.sides[edge] = match edges.sides[edge] {
                Sides::One {
                    corner: first,
                    next: first_next,
                } => {
                    let (here, there) = match self.point(first) == from {
                        true => ((first, corner), (first_next, next)),
                        false => ((first, next), (first_next, corner)),
                    };
                    let smooth =
                        self.same_normal(normals, here) && self.same_normal(normals, there);
                    Sides::Two { smooth }
                }
                Sides::Two { .. } | Sides::Many => Sides::Many,
            };
        }
        edges
    }

    fn same_normal(&self, normals: &[Vec3], (one, two): (usize, usize)) -> bool {
        let (one, two) = (self.corners[one] as usize, self.corners[two] as usize);
        match (normals.get(one), normals.get(two)) {
            (Some(one), Some(two)) => one.abs_diff_eq(*two, SMOOTH_NORMAL_TOLERANCE),
            _ => true,
        }
    }
}

#[derive(Clone, Copy)]
enum Sides {
    One { corner: usize, next: usize },
    Two { smooth: bool },
    Many,
}

#[derive(Default)]
pub(crate) struct Edges {
    pub first_corners: Vec<i32>,
    sides: Vec<Sides>,
}

impl Edges {
    pub(crate) fn smoothing(&self) -> Vec<i32> {
        self.sides
            .iter()
            .map(|sides| match sides {
                Sides::One { .. } | Sides::Two { smooth: true } => 1,
                Sides::Two { smooth: false } | Sides::Many => 0,
            })
            .collect()
    }
}
