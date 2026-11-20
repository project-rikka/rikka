mod constraint;
pub(crate) mod kernel;
mod label;
mod plane;
pub(crate) mod triangulation;

use glam::DVec2;
use rustc_hash::FxHashMap;

use crate::error::{Error, Result};
use crate::merge::{MergeRules, merge_triangles};
use kernel::Kernel;
use plane::PlaneKernel;
use triangulation::{Triangulation, edge_key};

pub(crate) use label::FaceSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CdtOutput {
    Full,
    Inside,
    InsideWithHoles,
    InsideWithHolesNonzero,
    Constraints,
    ConstraintsValidBmesh,
    ConstraintsValidBmeshWithHoles,
    ConstraintsValidBmeshWithHolesNonzero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EdgeOrigin {
    Edge(u32),
    FaceEdge { face: u32, position: u32 },
}

#[must_use]
#[derive(Clone, Debug, Default)]
pub struct Cdt {
    verts: Vec<DVec2>,
    edges: Vec<[u32; 2]>,
    faces: Vec<Vec<u32>>,
    epsilon: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CdtResult {
    pub verts: Vec<DVec2>,
    pub edges: Vec<[u32; 2]>,
    pub faces: Vec<Vec<u32>>,
    pub vert_origins: Vec<Vec<u32>>,
    pub edge_origins: Vec<Vec<EdgeOrigin>>,
    pub face_origins: Vec<Vec<u32>>,
}

impl Cdt {
    pub fn new(verts: impl Into<Vec<DVec2>>) -> Cdt {
        Cdt {
            verts: verts.into(),
            ..Cdt::default()
        }
    }

    pub fn edges(mut self, edges: impl Into<Vec<[u32; 2]>>) -> Cdt {
        self.edges = edges.into();
        self
    }

    pub fn faces<F: Into<Vec<u32>>>(mut self, faces: impl IntoIterator<Item = F>) -> Cdt {
        self.faces = faces.into_iter().map(Into::into).collect();
        self
    }

    pub fn epsilon(mut self, epsilon: f64) -> Cdt {
        self.epsilon = epsilon;
        self
    }

    fn validate(&self) -> Result<()> {
        if let Some(index) = self.verts.iter().position(|vert| !vert.is_finite()) {
            return Err(Error::NonFiniteVertex { index });
        }
        let len = self.verts.len();
        let check = |index: u32| match (index as usize) < len {
            true => Ok(()),
            false => Err(Error::VertexIndex { index, len }),
        };
        for edge in &self.edges {
            check(edge[0])?;
            check(edge[1])?;
        }
        for (face, verts) in self.faces.iter().enumerate() {
            if verts.len() < 3 {
                return Err(Error::FaceTooSmall { face });
            }
            for &vert in verts {
                check(vert)?;
            }
        }
        if !(self.epsilon >= 0.0 && self.epsilon.is_finite()) {
            return Err(Error::InvalidEpsilon(self.epsilon));
        }
        Ok(())
    }

    fn merged_verts(&self) -> Vec<u32> {
        let mut representative: Vec<u32> = (0..self.verts.len() as u32).collect();
        if self.epsilon <= 0.0 {
            return representative;
        }
        let cell = self.epsilon;
        let key = |point: DVec2| {
            (
                (point.x / cell).floor() as i64,
                (point.y / cell).floor() as i64,
            )
        };
        let mut grid: FxHashMap<(i64, i64), Vec<u32>> = FxHashMap::default();
        for (index, &point) in self.verts.iter().enumerate() {
            let (x, y) = key(point);
            let mut found: Option<u32> = None;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    let Some(list) = grid.get(&(x + dx, y + dy)) else {
                        continue;
                    };
                    for &other in list {
                        if self.verts[other as usize].distance(point) <= self.epsilon {
                            found = Some(found.map_or(other, |best| best.min(other)));
                        }
                    }
                }
            }
            match found {
                Some(target) => representative[index] = representative[target as usize],
                None => grid.entry((x, y)).or_default().push(index as u32),
            }
        }
        representative
    }

    pub fn triangulate(&self, output: CdtOutput) -> Result<CdtResult> {
        self.validate()?;
        let merged = self.merged_verts();
        let kernel = PlaneKernel::new(&self.verts);
        let originals = self.verts.len();
        let mut order: Vec<u32> = (0..originals as u32)
            .filter(|&index| merged[index as usize] == index)
            .collect();
        kernel.sort_spatially(&mut order);
        let mut triangulation = Triangulation::new(kernel);
        let built = triangulation.build(&order);
        let representative: Vec<u32> = (0..originals)
            .map(|index| built[merged[index] as usize])
            .collect();
        for (index, edge) in self.edges.iter().enumerate() {
            let (a, b) = (
                representative[edge[0] as usize],
                representative[edge[1] as usize],
            );
            if a != b {
                triangulation.insert_segment(a, b, (a, b), EdgeOrigin::Edge(index as u32));
            }
        }
        for (face, verts) in self.faces.iter().enumerate() {
            for position in 0..verts.len() {
                let (p, q) = (verts[position], verts[(position + 1) % verts.len()]);
                let (a, b) = (representative[p as usize], representative[q as usize]);
                if a != b {
                    let origin = EdgeOrigin::FaceEdge {
                        face: face as u32,
                        position: position as u32,
                    };
                    triangulation.insert_segment(a, b, (a, b), origin);
                }
            }
        }
        Ok(extract(&triangulation, &representative, originals, output))
    }
}

fn kept_triangles<K: Kernel>(
    triangulation: &Triangulation<K>,
    membership: &FxHashMap<u32, FaceSet>,
    output: CdtOutput,
) -> Vec<u32> {
    let real = triangulation.real_triangles();
    match output {
        CdtOutput::Full => real,
        CdtOutput::Inside => {
            let outside = triangulation.outside_reachable();
            real.into_iter()
                .filter(|&tri| !outside[tri as usize])
                .collect()
        }
        CdtOutput::InsideWithHoles | CdtOutput::InsideWithHolesNonzero => {
            let nonzero = output == CdtOutput::InsideWithHolesNonzero;
            let values = triangulation.crossing_values(nonzero);
            real.into_iter()
                .filter(|&tri| inside_value(values[tri as usize], nonzero))
                .collect()
        }
        CdtOutput::Constraints | CdtOutput::ConstraintsValidBmesh => real
            .into_iter()
            .filter(|tri| membership.contains_key(tri))
            .collect(),
        CdtOutput::ConstraintsValidBmeshWithHoles
        | CdtOutput::ConstraintsValidBmeshWithHolesNonzero => {
            let nonzero = output == CdtOutput::ConstraintsValidBmeshWithHolesNonzero;
            let values = triangulation.crossing_values(nonzero);
            real.into_iter()
                .filter(|&tri| {
                    membership.contains_key(&tri) && inside_value(values[tri as usize], nonzero)
                })
                .collect()
        }
    }
}

fn inside_value(value: i64, nonzero: bool) -> bool {
    match nonzero {
        true => value != 0 && value != i64::MIN,
        false => value != i64::MIN && value.rem_euclid(2) == 1,
    }
}

fn extract<K: Kernel>(
    triangulation: &Triangulation<K>,
    representative: &[u32],
    originals: usize,
    output: CdtOutput,
) -> CdtResult {
    let mut result = CdtResult::default();
    let mut index_of: Vec<u32> = vec![u32::MAX; triangulation.kernel.vertex_count()];
    for (input, &rep) in representative.iter().enumerate().take(originals) {
        if index_of[rep as usize] == u32::MAX {
            index_of[rep as usize] = result.verts.len() as u32;
            result.verts.push(triangulation.kernel.approx(rep));
            result.vert_origins.push(Vec::new());
        }
        result.vert_origins[index_of[rep as usize] as usize].push(input as u32);
    }
    for (vertex, slot) in index_of.iter_mut().enumerate().skip(originals) {
        *slot = result.verts.len() as u32;
        result
            .verts
            .push(triangulation.kernel.approx(vertex as u32));
        result.vert_origins.push(Vec::new());
    }
    let mut edge_index: FxHashMap<(u32, u32), u32> = FxHashMap::default();
    let mut push_edge = |result: &mut CdtResult, a: u32, b: u32| {
        let key = edge_key(a, b);
        if edge_index.contains_key(&key) {
            return;
        }
        edge_index.insert(key, result.edges.len() as u32);
        result
            .edges
            .push([index_of[a as usize], index_of[b as usize]]);
        let mut origins: Vec<EdgeOrigin> = triangulation
            .constraints
            .get(&key)
            .map(|info| info.origins.iter().map(|origin| origin.kind).collect())
            .unwrap_or_default();
        origins.sort_unstable();
        origins.dedup();
        result.edge_origins.push(origins);
    };
    if let Some(line) = &triangulation.line {
        if output == CdtOutput::Full {
            for pair in line.windows(2) {
                push_edge(&mut result, pair[0], pair[1]);
            }
        }
    } else {
        let membership = triangulation.face_membership();
        let kept = kept_triangles(triangulation, &membership, output);
        let constraint_mode = matches!(
            output,
            CdtOutput::Constraints
                | CdtOutput::ConstraintsValidBmesh
                | CdtOutput::ConstraintsValidBmeshWithHoles
                | CdtOutput::ConstraintsValidBmeshWithHolesNonzero
        );
        let faces: Vec<(Vec<u32>, Vec<u32>)> = match constraint_mode {
            false => kept
                .iter()
                .map(|tri| {
                    let set = membership
                        .get(tri)
                        .map(|set| set.to_vec())
                        .unwrap_or_default();
                    (triangulation.tris[*tri as usize].v.to_vec(), set)
                })
                .collect(),
            true => merge_groups(
                triangulation,
                &membership,
                &kept,
                output == CdtOutput::Constraints,
            ),
        };
        for (verts, origins) in faces {
            for i in 0..verts.len() {
                push_edge(&mut result, verts[i], verts[(i + 1) % verts.len()]);
            }
            result
                .faces
                .push(verts.iter().map(|&v| index_of[v as usize]).collect());
            result.face_origins.push(origins);
        }
    }
    let mut constrained: Vec<&(u32, u32)> = triangulation.constraints.keys().collect();
    constrained.sort_unstable_by_key(|key| {
        (
            index_of[key.0 as usize].min(index_of[key.1 as usize]),
            index_of[key.0 as usize].max(index_of[key.1 as usize]),
        )
    });
    for key in constrained {
        push_edge(&mut result, key.0, key.1);
    }
    result
}

fn merge_groups<K: Kernel>(
    triangulation: &Triangulation<K>,
    membership: &FxHashMap<u32, FaceSet>,
    kept: &[u32],
    allow_repeated: bool,
) -> Vec<(Vec<u32>, Vec<u32>)> {
    let mut groups: Vec<(FaceSet, Vec<[u32; 3]>)> = Vec::new();
    let mut group_of: FxHashMap<FaceSet, usize> = FxHashMap::default();
    for tri in kept {
        let set = membership[tri].clone();
        let index = *group_of.entry(set.clone()).or_insert_with(|| {
            groups.push((set, Vec::new()));
            groups.len() - 1
        });
        groups[index].1.push(triangulation.tris[*tri as usize].v);
    }
    let mut faces = Vec::new();
    for (set, triangles) in groups {
        let rules = MergeRules {
            dissolvable: |a: u32, b: u32| !triangulation.is_constrained(a, b),
            length_squared: |a: u32, b: u32| {
                (triangulation.kernel.approx(a) - triangulation.kernel.approx(b)).length_squared()
            },
            allow_repeated_verts: allow_repeated,
        };
        for polygon in merge_triangles(&triangles, rules) {
            faces.push((polygon, set.to_vec()));
        }
    }
    faces
}
