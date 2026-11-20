use glam::Vec3;
use rikka_mesh_edit::{EdgeId, FaceId, LoopId, VertId};
use rustc_hash::FxHashMap;

use crate::error::Result;
use crate::math::{
    AxisProjection, EPSILON_BIG, closest_to_segment, distance_squared_to_segment, point_in_polygon,
    polygon_area,
};
use crate::model::{BevId, BoundId, FaceKind};
use crate::state::{Bevel, Interpolation};

const VALUE_COUNT: usize = 6;

pub(crate) struct NgonSpec<'a> {
    pub(crate) verts: &'a [VertId],
    pub(crate) face_reps: Option<&'a [Option<FaceId>]>,
    pub(crate) facerep: Option<FaceId>,
    pub(crate) snap_edges: Option<&'a [Option<EdgeId>]>,
    pub(crate) bev_vert: Option<VertId>,
    pub(crate) origins: Option<&'a FxHashMap<VertId, VertId>>,
    pub(crate) material: Option<i32>,
}

impl<'a> NgonSpec<'a> {
    pub(crate) fn new(verts: &'a [VertId]) -> NgonSpec<'a> {
        NgonSpec {
            verts,
            face_reps: None,
            facerep: None,
            snap_edges: None,
            bev_vert: None,
            origins: None,
            material: None,
        }
    }
}

impl Bevel<'_> {
    pub(crate) fn boundvert_rep_face(&self, bound: BoundId) -> (Option<FaceId>, Option<FaceId>) {
        let state = self.bound(bound);
        let mut frep2 = None;
        let frep;
        if let Some(ebev) = state.ebev {
            frep = self.half(ebev).fprev;
            let efirst = state.efirst.and_then(|half| self.half(half).fprev);
            if efirst != frep {
                frep2 = efirst;
            }
        } else if let Some(efirst) = state.efirst {
            let first = self.half(efirst);
            let last = state.elast.map(|half| self.half(half).clone());
            let last_fnext = last.as_ref().and_then(|half| half.fnext);
            let last_fprev = last.as_ref().and_then(|half| half.fprev);
            if let Some(face) = first.fprev {
                frep = Some(face);
                if last_fnext != frep {
                    frep2 = last_fnext;
                } else if first.fnext != frep {
                    frep2 = first.fnext;
                } else if last_fprev != frep {
                    frep2 = first.fprev;
                }
            } else if let Some(face) = first.fnext {
                frep = Some(face);
                if last_fnext != frep {
                    frep2 = last_fnext;
                }
            } else {
                frep = last_fprev;
            }
        } else if let Some(prev_last) = self.bound(state.prev).elast {
            let mut chosen = self.half(prev_last).fnext;
            if let Some(next_first) = self.bound(state.next).efirst {
                match chosen {
                    Some(_) => frep2 = self.half(next_first).fprev,
                    None => chosen = self.half(next_first).fprev,
                }
            }
            frep = chosen;
        } else {
            frep = None;
        }
        (frep, frep2)
    }

    pub(crate) fn choose_rep_face(&self, faces: &[Option<FaceId>]) -> Option<FaceId> {
        let count = faces.len();
        let mut values = vec![[0.0f32; VALUE_COUNT]; count];
        let mut viable = vec![false; count];
        let mut num_viable = 0;
        for (index, face) in faces.iter().enumerate() {
            let Some(face) = *face else {
                continue;
            };
            viable[index] = true;
            num_viable += 1;
            let center = self.mesh.face_center_bounds(face);
            let material = self.mesh.face_material(face);
            values[index] = [
                self.uv.face_component(face).unwrap_or(0.0),
                match self.mesh.is_face_selected(face) {
                    true => 0.0,
                    false => 1.0,
                },
                match material >= 0 {
                    true => material as f32,
                    false => 0.0,
                },
                center.z,
                center.x,
                center.y,
            ];
        }
        let mut best: Option<usize> = None;
        let mut value_index = 0;
        while num_viable > 1 && value_index < VALUE_COUNT {
            for f in 0..count {
                if !viable[f] || Some(f) == best {
                    continue;
                }
                let Some(current) = best else {
                    best = Some(f);
                    continue;
                };
                if values[f][value_index] < values[current][value_index] {
                    best = Some(f);
                    for i in (0..f).rev() {
                        if viable[i] {
                            viable[i] = false;
                            num_viable -= 1;
                        }
                    }
                } else if values[f][value_index] > values[current][value_index] {
                    viable[f] = false;
                    num_viable -= 1;
                }
            }
            value_index += 1;
        }
        faces[best.unwrap_or(0)]
    }

    pub(crate) fn incident_edges(
        &self,
        face: Option<FaceId>,
        vert: VertId,
    ) -> (Option<EdgeId>, Option<EdgeId>) {
        let Some(face) = face else {
            return (None, None);
        };
        let mut found = (None, None);
        for edge in self.mesh.face_edges(face) {
            if self.mesh.edge_has_vert(edge, vert) {
                if found.0.is_none() {
                    found.0 = Some(edge);
                } else if found.1.is_none() {
                    found.1 = Some(edge);
                }
            }
        }
        found
    }

    pub(crate) fn closer_edge(
        &self,
        co: Vec3,
        e1: Option<EdgeId>,
        e2: Option<EdgeId>,
    ) -> Option<EdgeId> {
        let (Some(e1), Some(e2)) = (e1, e2) else {
            return e1.or(e2);
        };
        let distance = |edge: EdgeId| {
            let [a, b] = self.mesh.edge_verts(edge);
            distance_squared_to_segment(co, self.position(a), self.position(b))
        };
        match distance(e1) < distance(e2) {
            true => Some(e1),
            false => Some(e2),
        }
    }

    pub(crate) fn face_contains_point(&self, face: FaceId, co: Vec3) -> bool {
        let projection = AxisProjection::new(self.face_normal(face));
        let polygon: Vec<_> = self
            .mesh
            .face_verts(face)
            .map(|vert| projection.apply(self.position(vert)))
            .collect();
        point_in_polygon(projection.apply(co), &polygon)
    }

    pub(crate) fn face_internal_bounds(&self, bev: BevId, face: Option<FaceId>) -> Vec<BoundId> {
        let Some(face) = face else {
            return Vec::new();
        };
        let mut internal = Vec::new();
        for bound in self.bev_ring(bev) {
            if self.face_contains_point(face, self.bound(bound).co) {
                internal.push(bound);
                if internal.len() == 3 {
                    break;
                }
            }
        }
        internal
    }

    fn projected_boundary_area(&self, bev: BevId, face: FaceId) -> f32 {
        let projection = AxisProjection::new(self.face_normal(face));
        let vert = self.bev(bev).vert;
        let (e1, e2) = self.incident_edges(Some(face), vert);
        let (Some(e1), Some(e2)) = (e1, e2) else {
            return 0.0;
        };
        let unsnapped = self.face_internal_bounds(bev, Some(face));
        let segment = |edge: EdgeId| {
            let [a, b] = self.mesh.edge_verts(edge);
            (self.position(a), self.position(b))
        };
        let (a1, b1) = segment(e1);
        let (a2, b2) = segment(e2);
        let points: Vec<_> = self
            .bev_ring(bev)
            .into_iter()
            .map(|bound| {
                let co = self
                    .bound(bound)
                    .vert
                    .map_or(self.bound(bound).co, |vert| self.position(vert));
                if unsnapped.contains(&bound) {
                    return projection.apply(co);
                }
                let snap1 = closest_to_segment(co, a1, b1);
                let snap2 = closest_to_segment(co, a2, b2);
                match snap1.distance_squared(co) <= snap2.distance_squared(co) {
                    true => projection.apply(snap1),
                    false => projection.apply(snap2),
                }
            })
            .collect();
        polygon_area(&points)
    }

    pub(crate) fn frep_for_center_poly(&self, bev: BevId) -> Option<FaceId> {
        let state = self.bev(bev);
        let consider_all = state.selcount == 1 || self.params.affect_vertices_odd;
        let mut any = None;
        let mut choices: Vec<Option<FaceId>> = Vec::new();
        for half in state.halves() {
            let entry = self.half(half);
            if !entry.is_bev && !consider_all {
                continue;
            }
            let Some(face) = self.choose_rep_face(&[entry.fprev, entry.fnext]) else {
                continue;
            };
            any.get_or_insert(face);
            if choices.contains(&Some(face)) {
                continue;
            }
            if self.uv.has_math_layers() && self.projected_boundary_area(bev, face) < EPSILON_BIG {
                continue;
            }
            choices.push(Some(face));
        }
        match choices.is_empty() {
            true => any,
            false => self.choose_rep_face(&choices),
        }
    }

    fn interpolation(&mut self, face: FaceId) -> &Interpolation {
        if !self.interpolations.contains(face) {
            let interpolation = Interpolation {
                sample: self.mesh.sample_face(face),
                loops: self.mesh.face_loops_vec(face),
                verts: self.mesh.face_verts_vec(face),
            };
            self.interpolations.insert(face, interpolation);
        }
        self.interpolations
            .get(face)
            .expect("interpolation was cached")
    }

    pub(crate) fn interpolate_corner(&mut self, lp: LoopId, source: FaceId, snap: Option<EdgeId>) {
        let vert = self.mesh.loop_vert(lp);
        let mut point = self.position(vert);
        if let Some(edge) = snap {
            let [a, b] = self.mesh.edge_verts(edge);
            point = closest_to_segment(point, self.position(a), self.position(b));
        }
        let interpolation = self.interpolation(source);
        let weights = interpolation.sample.weights(point);
        let loops: Vec<(LoopId, f32)> = weights
            .iter()
            .map(|&(index, weight)| (interpolation.loops[index as usize], weight))
            .collect();
        let verts: Vec<(VertId, f32)> = weights
            .iter()
            .map(|&(index, weight)| (interpolation.verts[index as usize], weight))
            .collect();
        self.mesh.interpolate_loop_data(&loops, lp);
        self.mesh.interpolate_vert_data(&verts, vert);
    }

    pub(crate) fn create_ngon(&mut self, spec: &NgonSpec) -> Result<Option<FaceId>> {
        let mut builder = self.mesh.new_face(spec.verts);
        if let Some(facerep) = spec.facerep {
            builder = builder.example(facerep);
        }
        let face = match builder.create() {
            Ok(face) => face,
            Err(_) => return Ok(None),
        };
        let source = spec.facerep.or_else(|| {
            spec.face_reps
                .and_then(|reps| reps.first().copied().flatten())
        });
        if let Some(source) = source {
            self.mesh.copy_face_data(source, face);
            let loops = self.mesh.face_loops_vec(face);
            for (index, lp) in loops.into_iter().enumerate() {
                let interp = match spec.face_reps {
                    Some(reps) => reps.get(index).copied().flatten(),
                    None => spec.facerep,
                };
                if let Some(interp) = interp {
                    let snap = spec
                        .snap_edges
                        .and_then(|edges| edges.get(index).copied().flatten());
                    self.interpolate_corner(lp, interp, snap);
                }
            }
        }
        self.mesh.set_face_tag(face, true);
        let edges = self.mesh.face_edges_vec(face);
        self.out_edges.extend(edges);
        if let Some(material) = spec.material {
            self.mesh.set_face_material(face, material);
        }
        self.register_uv_face(
            face,
            spec.facerep,
            spec.face_reps,
            spec.bev_vert,
            spec.origins,
        );
        Ok(Some(face))
    }

    pub(crate) fn record(&mut self, face: Option<FaceId>, kind: FaceKind) {
        if let Some(face) = face {
            self.record_face_kind(face, kind);
        }
    }
}
