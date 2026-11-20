use std::collections::hash_map::Entry;

use glam::Vec2;
use rikka_mesh_edit::{EdgeId, FaceId, IdMap, LoopId, MathLayer, MathLayerKind, VertId};
use rikka_polymesh::{Domain, Values};
use rustc_hash::FxHashMap;

use crate::state::Bevel;

const UV_CONNECT_LIMIT: f32 = 0.0001;

type Bucket = Vec<LoopId>;

#[derive(Clone, Debug, Default)]
pub(crate) struct UvState {
    has_layers: bool,
    math_layers: Vec<MathLayer>,
    uv_layers: Vec<String>,
    has_math_layers: bool,
    face_component: Option<Vec<i32>>,
    attached: IdMap<FaceId, Option<FaceId>>,
    vert_maps: Vec<FxHashMap<VertId, Vec<Bucket>>>,
}

impl UvState {
    pub(crate) fn has_math_layers(&self) -> bool {
        self.has_math_layers
    }

    pub(crate) fn face_component(&self, face: FaceId) -> Option<f32> {
        self.face_component
            .as_ref()
            .map(|components| components.get(face.index()).copied().unwrap_or(-1) as f32)
    }
}

impl Bevel<'_> {
    pub(crate) fn init_uv_state(&mut self) {
        let mut state = UvState::default();
        for attribute in self.mesh.attributes(Domain::Corner) {
            state.has_layers = true;
            match &attribute.values {
                Values::Vec2(_) => {
                    state.math_layers.push(MathLayer {
                        name: attribute.name.clone(),
                        kind: MathLayerKind::Vec2,
                    });
                    state.uv_layers.push(attribute.name.clone());
                }
                Values::Color(_) => state.math_layers.push(MathLayer {
                    name: attribute.name.clone(),
                    kind: MathLayerKind::Color,
                }),
                _ => {}
            }
        }
        state.has_math_layers = !state.uv_layers.is_empty();
        state.vert_maps = vec![FxHashMap::default(); state.uv_layers.len()];
        self.uv = state;
        if self.uv.has_math_layers && self.params.seg % 2 == 1 {
            self.uv.face_component = Some(self.face_components());
        }
    }

    fn math_equal(&self, a: LoopId, b: LoopId) -> bool {
        self.uv
            .math_layers
            .iter()
            .all(|layer| layer.loops_equal(self.mesh, a, b))
    }

    pub(crate) fn contig_ldata_across_edge(&self, edge: EdgeId, f1: FaceId, f2: FaceId) -> bool {
        if !self.uv.has_layers {
            return true;
        }
        let Some((mut lef1, mut lef2)) = self.mesh.edge_loop_pair(edge) else {
            return false;
        };
        if self.mesh.loop_face(lef1) == f2 {
            std::mem::swap(&mut lef1, &mut lef2);
        }
        if self.mesh.loop_face(lef1) != f1 || self.mesh.loop_face(lef2) != f2 {
            return false;
        }
        if self.mesh.loop_vert(lef1) == self.mesh.loop_vert(lef2) {
            return false;
        }
        let lv1f1 = lef1;
        let lv2f1 = self.mesh.loop_next(lef1);
        let lv1f2 = self.mesh.loop_next(lef2);
        let lv2f2 = lef2;
        self.math_equal(lv1f1, lv1f2) && self.math_equal(lv2f1, lv2f2)
    }

    pub(crate) fn contig_ldata_around_vert(&self, vert: VertId) -> bool {
        if !self.uv.has_layers {
            return true;
        }
        let mut loops = self.mesh.vert_loops(vert);
        let Some(first) = loops.next() else {
            return true;
        };
        loops.all(|lp| self.math_equal(first, lp))
    }

    fn face_components(&self) -> Vec<i32> {
        let faces: Vec<FaceId> = self.mesh.faces().collect();
        let mut index_of = vec![usize::MAX; self.mesh.face_capacity()];
        for (index, face) in faces.iter().enumerate() {
            index_of[face.index()] = index;
        }
        let total = faces.len();
        let mut component = vec![-1i32; total];
        let mut in_stack = vec![false; total];
        let mut current = -1;
        let mut stack: Vec<usize> = Vec::with_capacity(total);
        for start in 0..total {
            if component[start] != -1 || in_stack[start] {
                continue;
            }
            current += 1;
            stack.push(start);
            in_stack[start] = true;
            while let Some(index) = stack.pop() {
                in_stack[index] = false;
                if component[index] != -1 {
                    continue;
                }
                component[index] = current;
                let face = faces[index];
                for edge in self.mesh.face_edges(face) {
                    for other in self.mesh.edge_faces(edge) {
                        if other == face {
                            continue;
                        }
                        let other_index = index_of[other.index()];
                        if component[other_index] != -1 || in_stack[other_index] {
                            continue;
                        }
                        if self.contig_ldata_across_edge(edge, face, other) {
                            stack.push(other_index);
                            in_stack[other_index] = true;
                        }
                    }
                }
            }
        }
        if current > 0 {
            let mut top_z = -1.0e30f32;
            let mut top = -1;
            let mut bottom_z = 1.0e30f32;
            let mut bottom = -1;
            for (index, &face) in faces.iter().enumerate() {
                let z = self.mesh.face_center_bounds(face).z;
                if z > top_z {
                    top_z = z;
                    top = component[index];
                }
                if z < bottom_z {
                    bottom_z = z;
                    bottom = component[index];
                }
            }
            let first = component[0];
            swap_components(&mut component, first, top);
            if bottom != top {
                if bottom == 0 {
                    bottom = top;
                }
                let second = component[1];
                swap_components(&mut component, second, bottom);
            }
        }
        let mut by_slot = vec![-1i32; self.mesh.face_capacity()];
        for (index, face) in faces.iter().enumerate() {
            by_slot[face.index()] = component[index];
        }
        by_slot
    }

    pub(crate) fn determine_uv_connectivity(&mut self, vert: VertId) {
        let loops: Vec<LoopId> = self.mesh.vert_loops(vert).collect();
        for layer in 0..self.uv.uv_layers.len() {
            let Some(uvs) = self
                .mesh
                .attribute::<Vec2>(&self.uv.uv_layers[layer], Domain::Corner)
            else {
                continue;
            };
            let mut buckets: Vec<Bucket> = Vec::new();
            for &lp in &loops {
                let uv = uvs[lp.index()];
                let found = buckets.iter().position(|bucket| {
                    bucket.iter().any(|&other| {
                        let other = uvs[other.index()];
                        (uv.x - other.x).abs() <= UV_CONNECT_LIMIT
                            && (uv.y - other.y).abs() <= UV_CONNECT_LIMIT
                    })
                });
                match found {
                    Some(index) => buckets[index].push(lp),
                    None => buckets.push(vec![lp]),
                }
            }
            self.uv.vert_maps[layer].insert(vert, buckets);
        }
    }

    pub(crate) fn register_uv_face(
        &mut self,
        face: FaceId,
        facerep: Option<FaceId>,
        face_reps: Option<&[Option<FaceId>]>,
        bev_vert: Option<VertId>,
        origins: Option<&FxHashMap<VertId, VertId>>,
    ) {
        let attached = match face_reps.and_then(|reps| reps.first().copied().flatten()) {
            Some(first) => Some(first),
            None => facerep,
        };
        self.uv.attached.insert(face, attached);
        let Some(attached) = attached else {
            return;
        };
        let loops = self.mesh.face_loops_vec(face);
        for layer in 0..self.uv.vert_maps.len() {
            for &lp in &loops {
                let vert = self.mesh.loop_vert(lp);
                if let Entry::Vacant(entry) = self.uv.vert_maps[layer].entry(vert) {
                    entry.insert(vec![vec![lp]]);
                    continue;
                }
                let origin = match origins {
                    Some(map) => map.get(&vert).copied(),
                    None => bev_vert,
                };
                let orig_l =
                    origin.and_then(|origin| self.mesh.face_vert_share_loop(attached, origin));
                let mut found = false;
                for other in self.mesh.vert_loops(vert).collect::<Vec<_>>() {
                    if other == lp {
                        continue;
                    }
                    let Some(Some(attached2)) = self.uv.attached.copied(self.mesh.loop_face(other))
                    else {
                        continue;
                    };
                    let orig_l2 =
                        origin.and_then(|origin| self.mesh.face_vert_share_loop(attached2, origin));
                    let connected = match (origin, orig_l, orig_l2) {
                        (Some(origin), Some(a), Some(b)) => self.uv.vert_maps[layer]
                            .get(&origin)
                            .is_some_and(|buckets| {
                                buckets
                                    .iter()
                                    .any(|bucket| bucket.contains(&a) && bucket.contains(&b))
                            }),
                        _ => false,
                    };
                    if attached == attached2 || connected {
                        let buckets = self.uv.vert_maps[layer]
                            .get_mut(&vert)
                            .expect("vertex buckets exist");
                        if let Some(bucket) =
                            buckets.iter_mut().find(|bucket| bucket.contains(&other))
                        {
                            if !bucket.contains(&lp) {
                                bucket.push(lp);
                            }
                            found = true;
                        }
                    }
                    if found {
                        break;
                    }
                }
                if !found {
                    self.uv.vert_maps[layer]
                        .get_mut(&vert)
                        .expect("vertex buckets exist")
                        .push(vec![lp]);
                }
            }
        }
    }

    pub(crate) fn forget_uv_vert(&mut self, vert: VertId) {
        for map in &mut self.uv.vert_maps {
            map.remove(&vert);
        }
    }

    pub(crate) fn merge_uvs(&mut self) {
        for layer in 0..self.uv.uv_layers.len() {
            let name = self.uv.uv_layers[layer].clone();
            let buckets: Vec<Bucket> = self.uv.vert_maps[layer]
                .values()
                .flat_map(|buckets| buckets.iter().filter(|bucket| bucket.len() > 1).cloned())
                .collect();
            let Some(uvs) = self.mesh.attribute_mut::<Vec2>(&name, Domain::Corner) else {
                continue;
            };
            for bucket in buckets {
                let mut sum = Vec2::ZERO;
                for &lp in &bucket {
                    sum += uvs[lp.index()];
                }
                let average = sum * (1.0 / bucket.len() as f32);
                for &lp in &bucket {
                    uvs[lp.index()] = average;
                }
            }
        }
    }
}

fn swap_components(components: &mut [i32], a: i32, b: i32) {
    if a == b {
        return;
    }
    for component in components.iter_mut() {
        if *component == a {
            *component = b;
        } else if *component == b {
            *component = a;
        }
    }
}
