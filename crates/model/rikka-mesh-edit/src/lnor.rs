use std::f32::consts::TAU;

use glam::Vec3;
use rikka_polymesh::{CUSTOM_NORMAL, Domain, acos_approx};

use crate::ids::{EdgeId, ElementId, LoopId, VertId};
use crate::journal::JournalMark;
use crate::mesh::EditMesh;
use crate::parallel;
use crate::sets::IdSet;

const UNSET: u32 = u32::MAX;
const WHOLE_MESH_SHARE: usize = 4;
const BULK_SHARE: usize = 8;

const TRIGO_THRESHOLD: f32 = 1.0 - 1.0e-4;
const SAME_NORMAL: f32 = 1.0e-4;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NormalSpace {
    normal: Vec3,
    reference: Vec3,
    ortho: Vec3,
    alpha: f32,
    beta: f32,
}

fn unit_to_short(value: f32) -> i16 {
    (value * f32::from(i16::MAX) + 0.5).floor() as i16
}

fn short_to_unit(value: i16) -> f32 {
    f32::from(value) / f32::from(i16::MAX)
}

impl NormalSpace {
    #[must_use]
    pub fn define(normal: Vec3, reference: Vec3, other: Vec3, edges: &[Vec3]) -> NormalSpace {
        let reference_dot = reference.dot(normal);
        let other_dot = other.dot(normal);
        let mut space = NormalSpace {
            normal,
            ..NormalSpace::default()
        };
        if reference_dot.abs() >= TRIGO_THRESHOLD || other_dot.abs() >= TRIGO_THRESHOLD {
            return space;
        }
        space.alpha = match edges.is_empty() {
            false => {
                edges
                    .iter()
                    .map(|edge| acos_approx(edge.dot(normal)))
                    .sum::<f32>()
                    / edges.len() as f32
            }
            true => (acos_approx(reference_dot) + acos_approx(other_dot)) / 2.0,
        };
        space.reference = (reference - normal * reference_dot).normalize_or_zero();
        space.ortho = normal.cross(space.reference).normalize_or_zero();
        let projected = (other - normal * other_dot).normalize_or_zero();
        let dot = space.reference.dot(projected);
        space.beta = match dot < TRIGO_THRESHOLD {
            true => {
                let beta = acos_approx(dot);
                match space.ortho.dot(projected) < 0.0 {
                    true => TAU - beta,
                    false => beta,
                }
            }
            false => TAU,
        };
        space
    }

    #[must_use]
    pub fn normal(&self) -> Vec3 {
        self.normal
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.alpha != 0.0 && self.beta != 0.0
    }

    #[must_use]
    pub fn encode(&self, custom: Vec3) -> [i16; 2] {
        if custom == Vec3::ZERO
            || (self.normal - custom).abs().max_element() <= SAME_NORMAL
            || !self.is_valid()
        {
            return [0, 0];
        }
        let cos_alpha = self.normal.dot(custom);
        let alpha = acos_approx(cos_alpha);
        let first = match alpha > self.alpha {
            true => unit_to_short(-(TAU - alpha) / (TAU - self.alpha)),
            false => unit_to_short(alpha / self.alpha),
        };
        let projected = (self.normal * -cos_alpha + custom).normalize_or_zero();
        let cos_beta = self.reference.dot(projected);
        let second = match cos_beta < TRIGO_THRESHOLD {
            true => {
                let mut beta = acos_approx(cos_beta);
                if self.ortho.dot(projected) < 0.0 {
                    beta = TAU - beta;
                }
                match beta > self.beta {
                    true => unit_to_short(-(TAU - beta) / (TAU - self.beta)),
                    false => unit_to_short(beta / self.beta),
                }
            }
            false => 0,
        };
        [first, second]
    }

    #[must_use]
    pub fn decode(&self, data: [i16; 2]) -> Vec3 {
        if data[0] == 0 || !self.is_valid() {
            return self.normal;
        }
        let alpha_factor = short_to_unit(data[0]);
        let alpha = match alpha_factor > 0.0 {
            true => self.alpha,
            false => TAU - self.alpha,
        } * alpha_factor;
        let beta_factor = short_to_unit(data[1]);
        let mut normal = self.normal * alpha.cos();
        if beta_factor == 0.0 {
            normal += self.reference * alpha.sin();
        } else {
            let beta = match beta_factor > 0.0 {
                true => self.beta,
                false => TAU - self.beta,
            } * beta_factor;
            normal += self.reference * (alpha.sin() * beta.cos());
            normal += self.ortho * (alpha.sin() * beta.sin());
        }
        normal
    }
}

#[derive(Clone, Copy, Debug)]
struct Fan {
    space: NormalSpace,
    start: u32,
    len: u32,
    single: bool,
}

#[derive(Clone, Copy, Debug)]
struct Synced {
    mesh: u64,
    topology: u64,
    data: u64,
    mark: JournalMark,
}

impl Synced {
    fn new(mesh: &EditMesh) -> Synced {
        let revisions = mesh.revisions();
        Synced {
            mesh: revisions.mesh,
            topology: revisions.topology,
            data: revisions.data,
            mark: mesh.journal_mark(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct NormalSpaces {
    fan_of: Vec<u32>,
    fans: Vec<Fan>,
    loops: Vec<LoopId>,
    smooth: Vec<bool>,
    synced: Synced,
}

fn edge_smooth_flags(mesh: &EditMesh) -> Vec<bool> {
    let mut smooth = vec![false; mesh.edge_capacity()];
    for edge in mesh.edges() {
        let Some(first) = mesh.edge_first_loop(edge) else {
            continue;
        };
        let second = mesh.loop_radial_next(first);
        smooth[edge.index()] = mesh.loop_radial_next(second) == first
            && mesh.loop_vert(first) != mesh.loop_vert(second)
            && !mesh.is_edge_sharp(edge)
            && mesh.is_face_smooth(mesh.loop_face(first))
            && mesh.is_face_smooth(mesh.loop_face(second));
    }
    smooth
}

pub fn edge_smooth_topology(mesh: &EditMesh) -> Vec<bool> {
    let mut smooth = vec![false; mesh.edge_capacity()];
    for edge in mesh.edges() {
        if let Some((a, b)) = mesh.edge_loop_pair(edge) {
            smooth[edge.index()] =
                !mesh.is_edge_sharp(edge) && mesh.loop_vert(a) != mesh.loop_vert(b);
        }
    }
    smooth
}

pub fn is_cyclic_smooth_fan(
    mesh: &EditMesh,
    smooth: &[bool],
    tagged: &mut [bool],
    start: LoopId,
) -> bool {
    let mut pivot = start;
    let mut edge = mesh.loop_edge(start);
    tagged[start.index()] = true;
    loop {
        let Some(next) = mesh.vert_step_fan_loop(pivot, &mut edge) else {
            return false;
        };
        if !smooth[edge.index()] {
            return false;
        }
        if tagged[next.index()] {
            return next == start;
        }
        tagged[next.index()] = true;
        pivot = next;
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FanWalk {
    pub loops: Vec<LoopId>,
    pub edges: Vec<EdgeId>,
}

impl FanWalk {
    pub fn walk(&mut self, mesh: &EditMesh, smooth: &[bool], start: LoopId) {
        self.loops.clear();
        self.edges.clear();
        let origin = mesh.loop_edge(start);
        let mut pivot = start;
        let mut edge = origin;
        loop {
            let next = mesh.vert_step_fan_loop(pivot, &mut edge);
            if next.is_none() {
                edge = match mesh.loop_edge(pivot) == edge {
                    true => mesh.loop_edge(mesh.loop_prev(pivot)),
                    false => mesh.loop_edge(pivot),
                };
            }
            self.loops.push(pivot);
            self.edges.push(edge);
            if !smooth[edge.index()] || edge == origin || self.loops.len() > mesh.loop_count() {
                break;
            }
            match next {
                Some(next) => pivot = next,
                None => break,
            }
        }
    }
}

pub fn walk_fan(mesh: &EditMesh, smooth: &[bool], start: LoopId) -> FanWalk {
    let mut walk = FanWalk::default();
    walk.walk(mesh, smooth, start);
    walk
}

fn direction(mesh: &EditMesh, from: Vec3, vert: VertId) -> Vec3 {
    (mesh.position(vert) - from).normalize_or_zero()
}

fn single_space(mesh: &EditMesh, lp: LoopId) -> NormalSpace {
    let origin = mesh.position(mesh.loop_vert(lp));
    let next = direction(mesh, origin, mesh.loop_next_vert(lp));
    let previous = direction(mesh, origin, mesh.loop_vert(mesh.loop_prev(lp)));
    NormalSpace::define(mesh.face_normal(mesh.loop_face(lp)), next, previous, &[])
}

#[derive(Default)]
struct FanScratch {
    walk: FanWalk,
    edge_vectors: Vec<Vec3>,
}

fn smooth_space(
    mesh: &EditMesh,
    smooth: &[bool],
    lp: LoopId,
    scratch: &mut FanScratch,
) -> NormalSpace {
    let FanScratch { walk, edge_vectors } = scratch;
    walk.walk(mesh, smooth, lp);
    let pivot = mesh.loop_vert(lp);
    let origin = mesh.position(pivot);
    let reference = direction(mesh, origin, mesh.loop_next_vert(lp));
    edge_vectors.clear();
    edge_vectors.push(reference);
    let mut current = reference;
    let mut normal = Vec3::ZERO;
    let origin_edge = mesh.loop_edge(lp);
    let mut other = reference;
    for (&pivot_loop, &edge) in walk.loops.iter().zip(&walk.edges) {
        let far = mesh
            .edge_other_vert(edge, pivot)
            .expect("fan edges contain the pivot");
        let next = direction(mesh, origin, far);
        normal += mesh.face_normal(mesh.loop_face(pivot_loop)) * acos_approx(next.dot(current));
        if edge != origin_edge {
            edge_vectors.push(next);
        }
        current = next;
        other = next;
    }
    let mut normal = normal.normalize_or_zero();
    if normal == Vec3::ZERO {
        normal = mesh.vert_normal(pivot);
    }
    NormalSpace::define(normal, reference, other, edge_vectors)
}

fn same_bits(a: Vec3, b: Vec3) -> bool {
    a.to_array().map(f32::to_bits) == b.to_array().map(f32::to_bits)
}

fn region(mesh: &EditMesh, moved: &[VertId]) -> Option<Vec<VertId>> {
    let limit = mesh.vert_count() / WHOLE_MESH_SHARE;
    if moved.len() >= limit {
        return None;
    }
    let mut seen: IdSet<VertId> = IdSet::new();
    let mut region = Vec::new();
    for &vert in moved {
        if !mesh.contains_vert(vert) {
            continue;
        }
        for face in mesh.vert_faces(vert) {
            for corner in mesh.face_verts(face) {
                if seen.insert(corner) {
                    region.push(corner);
                }
            }
        }
        if region.len() >= limit {
            return None;
        }
    }
    Some(region)
}

impl NormalSpaces {
    #[must_use]
    pub fn new(mesh: &EditMesh) -> NormalSpaces {
        let smooth = edge_smooth_flags(mesh);
        let mut tagged = vec![false; mesh.loop_capacity()];
        let mut spaces = NormalSpaces {
            fan_of: vec![UNSET; mesh.loop_capacity()],
            fans: Vec::with_capacity(mesh.vert_count()),
            loops: Vec::with_capacity(mesh.loop_count()),
            smooth: Vec::new(),
            synced: Synced::new(mesh),
        };
        let mut walk = FanWalk::default();
        for face in mesh.faces() {
            for lp in mesh.face_loops(face) {
                if smooth[mesh.loop_edge(lp).index()]
                    && (tagged[lp.index()] || !is_cyclic_smooth_fan(mesh, &smooth, &mut tagged, lp))
                {
                    continue;
                }
                if !smooth[mesh.loop_edge(lp).index()]
                    && !smooth[mesh.loop_edge(mesh.loop_prev(lp)).index()]
                {
                    spaces.push_fan(NormalSpace::default(), &[lp], true);
                    continue;
                }
                walk.walk(mesh, &smooth, lp);
                spaces.push_fan_reversed(NormalSpace::default(), &walk.loops);
            }
        }
        for face in mesh.faces() {
            for lp in mesh.face_loops(face) {
                if spaces.fan_of[lp.index()] != UNSET {
                    continue;
                }
                spaces.push_fan(NormalSpace::default(), &[lp], true);
            }
        }
        spaces.smooth = smooth;
        spaces.refresh_all(mesh);
        spaces
    }

    fn push_fan(&mut self, space: NormalSpace, loops: &[LoopId], single: bool) {
        let index = self.fans.len() as u32;
        self.fans.push(Fan {
            space,
            start: self.loops.len() as u32,
            len: loops.len() as u32,
            single,
        });
        for &lp in loops {
            self.fan_of[lp.index()] = index;
        }
        self.loops.extend_from_slice(loops);
    }

    fn push_fan_reversed(&mut self, space: NormalSpace, loops: &[LoopId]) {
        let start = self.loops.len();
        self.push_fan(space, loops, false);
        self.loops[start..].reverse();
    }

    fn is_current(&self, mesh: &EditMesh) -> bool {
        let revisions = mesh.revisions();
        (self.synced.mesh, self.synced.topology) == (revisions.mesh, revisions.topology)
            && self.fan_of.len() == mesh.loop_capacity()
            && self.smooth.len() == mesh.edge_capacity()
    }

    fn compute_space(&self, mesh: &EditMesh, fan: usize, scratch: &mut FanScratch) -> NormalSpace {
        let loops = self.fan_loops(fan);
        match self.fans[fan].single {
            true => single_space(mesh, loops[0]),
            false => smooth_space(mesh, &self.smooth, loops[loops.len() - 1], scratch),
        }
    }

    fn fans_around(&self, mesh: &EditMesh, verts: &[VertId]) -> Vec<u32> {
        let mut fans = Vec::new();
        let mut local: Vec<u32> = Vec::new();
        for &vert in verts {
            local.clear();
            local.extend(
                mesh.vert_loops(vert)
                    .map(|lp| self.fan_of[lp.index()])
                    .filter(|&fan| fan != UNSET),
            );
            local.sort_unstable();
            local.dedup();
            fans.extend_from_slice(&local);
        }
        fans
    }

    fn refresh(&mut self, mesh: &EditMesh, fans: &[u32]) {
        let spaces = parallel::concat(fans.len(), |range| {
            let mut scratch = FanScratch::default();
            range
                .map(|index| self.compute_space(mesh, fans[index] as usize, &mut scratch))
                .collect()
        });
        for (&fan, space) in fans.iter().zip(spaces) {
            self.fans[fan as usize].space = space;
        }
    }

    fn refresh_all(&mut self, mesh: &EditMesh) {
        let spaces = parallel::concat(self.fans.len(), |range| {
            let mut scratch = FanScratch::default();
            range
                .map(|fan| self.compute_space(mesh, fan, &mut scratch))
                .collect()
        });
        for (fan, space) in self.fans.iter_mut().zip(spaces) {
            fan.space = space;
        }
    }

    pub fn update_around(&mut self, mesh: &EditMesh, moved: &[VertId]) {
        if !self.is_current(mesh) {
            *self = NormalSpaces::new(mesh);
            return;
        }
        match region(mesh, moved) {
            Some(region) => {
                let fans = self.fans_around(mesh, &region);
                self.refresh(mesh, &fans);
            }
            None => self.refresh_all(mesh),
        }
        self.synced = Synced::new(mesh);
    }

    pub fn sync(&mut self, mesh: &EditMesh) {
        let changes = (self.is_current(mesh) && self.synced.data == mesh.revisions().data)
            .then(|| mesh.changes_since(self.synced.mark))
            .flatten();
        match changes {
            Some(changes) => self.update_around(mesh, changes.moved),
            None => *self = NormalSpaces::new(mesh),
        }
    }

    #[must_use]
    pub fn fan_count(&self) -> usize {
        self.fans.len()
    }

    #[must_use]
    pub fn fan_index(&self, lp: LoopId) -> usize {
        self.fan_of[lp.index()] as usize
    }

    #[must_use]
    pub fn fan_loops(&self, fan: usize) -> &[LoopId] {
        let fan = &self.fans[fan];
        &self.loops[fan.start as usize..(fan.start + fan.len) as usize]
    }

    #[must_use]
    pub fn fan_space(&self, fan: usize) -> &NormalSpace {
        &self.fans[fan].space
    }

    #[must_use]
    pub fn space(&self, lp: LoopId) -> &NormalSpace {
        &self.fans[self.fan_index(lp)].space
    }

    #[must_use]
    pub fn is_single(&self, lp: LoopId) -> bool {
        self.fans[self.fan_index(lp)].single
    }

    #[must_use]
    pub fn loops_sharing_space(&self, lp: LoopId) -> &[LoopId] {
        self.fan_loops(self.fan_index(lp))
    }

    pub fn encode_layer(&self, mesh: &EditMesh) -> Vec<[i16; 2]> {
        let mut data = vec![[0i16; 2]; mesh.loop_capacity()];
        let Some(stored) = mesh.custom_normals() else {
            return data;
        };
        let values = parallel::concat(self.fans.len(), |range| {
            let mut values: Vec<[i16; 2]> = Vec::new();
            for fan in range {
                let space = &self.fans[fan].space;
                let loops = self.fan_loops(fan);
                let start = values.len();
                let mut sum = [0i32; 2];
                let mut uniform = true;
                for &lp in loops {
                    let encoded = space.encode(stored[lp.index()]);
                    sum[0] += i32::from(encoded[0]);
                    sum[1] += i32::from(encoded[1]);
                    uniform &= values.len() == start || values[start] == encoded;
                    values.push(encoded);
                }
                if !uniform {
                    let count = loops.len() as i32;
                    let average = [(sum[0] / count) as i16, (sum[1] / count) as i16];
                    values[start..].fill(average);
                }
            }
            values
        });
        for (&lp, value) in self.loops.iter().zip(values) {
            data[lp.index()] = value;
        }
        data
    }

    pub fn decode_layer(&self, mesh: &mut EditMesh, data: &[[i16; 2]]) {
        let loops: Vec<LoopId> = mesh.loops().collect();
        let stored = mesh.custom_normals_mut();
        for lp in loops {
            if self.fan_of[lp.index()] == UNSET {
                continue;
            }
            stored[lp.index()] = self.space(lp).decode(data[lp.index()]);
        }
    }

    #[must_use]
    pub fn corner_normals(&self, mesh: &EditMesh) -> Vec<Vec3> {
        let mut normals = vec![Vec3::ZERO; mesh.loop_capacity()];
        let custom = mesh.custom_normals().map(|_| self.encode_layer(mesh));
        for fan in 0..self.fans.len() {
            let space = &self.fans[fan].space;
            let loops = self.fan_loops(fan);
            let normal = match &custom {
                Some(data) => space.decode(data[loops[0].index()]),
                None => space.normal,
            };
            for &lp in loops {
                normals[lp.index()] = normal;
            }
        }
        normals
    }

    fn decoded(&self, lp: LoopId, data: &[[i16; 2]]) -> Option<Vec3> {
        let fan = *self.fan_of.get(lp.index())?;
        if fan == UNSET {
            return None;
        }
        let encoded = data.get(lp.index()).copied().unwrap_or([0, 0]);
        Some(self.fans[fan as usize].space.decode(encoded))
    }

    fn changed_all(&self, mesh: &EditMesh, data: &[[i16; 2]]) -> Vec<(LoopId, Vec3)> {
        let current = mesh.custom_normals();
        parallel::concat(self.fan_of.len(), |range| {
            range
                .filter_map(|index| {
                    let lp = LoopId::from_index(index);
                    let value = self.decoded(lp, data)?;
                    let same = current.is_some_and(|current| same_bits(current[index], value));
                    (!same).then_some((lp, value))
                })
                .collect()
        })
    }

    fn changed_at(
        &self,
        mesh: &EditMesh,
        data: &[[i16; 2]],
        loops: impl Iterator<Item = LoopId>,
    ) -> Vec<(LoopId, Vec3)> {
        let current = mesh.custom_normals();
        loops
            .filter_map(|lp| {
                let value = self.decoded(lp, data)?;
                let same = current.is_some_and(|current| same_bits(current[lp.index()], value));
                (!same).then_some((lp, value))
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct EncodedCustomNormals {
    data: Vec<[i16; 2]>,
    spaces: NormalSpaces,
    unsettled: Vec<LoopId>,
}

impl EncodedCustomNormals {
    #[must_use]
    pub fn capture(mesh: &EditMesh) -> Option<EncodedCustomNormals> {
        EncodedCustomNormals::has_layer(mesh)
            .then(|| EncodedCustomNormals::encode(mesh, NormalSpaces::new(mesh)))
    }

    #[must_use]
    pub fn capture_with(mesh: &EditMesh, mut spaces: NormalSpaces) -> Option<EncodedCustomNormals> {
        if !EncodedCustomNormals::has_layer(mesh) {
            return None;
        }
        spaces.sync(mesh);
        Some(EncodedCustomNormals::encode(mesh, spaces))
    }

    fn has_layer(mesh: &EditMesh) -> bool {
        mesh.attribute_domain(CUSTOM_NORMAL) == Some(Domain::Corner)
            && mesh.custom_normals().is_some()
    }

    fn encode(mesh: &EditMesh, spaces: NormalSpaces) -> EncodedCustomNormals {
        let data = spaces.encode_layer(mesh);
        let unsettled = spaces
            .changed_all(mesh, &data)
            .into_iter()
            .map(|(lp, _)| lp)
            .collect();
        EncodedCustomNormals {
            data,
            spaces,
            unsettled,
        }
    }

    #[must_use]
    pub fn into_spaces(self) -> NormalSpaces {
        self.spaces
    }

    #[must_use]
    pub fn get(&self, lp: LoopId) -> [i16; 2] {
        self.data.get(lp.index()).copied().unwrap_or([0, 0])
    }

    pub fn restore(&self, mesh: &mut EditMesh) {
        let changed = NormalSpaces::new(mesh).changed_all(mesh, &self.data);
        mesh.set_custom_normals(&changed);
    }

    pub fn restore_around(&mut self, mesh: &mut EditMesh, moved: &[VertId]) {
        let changed = match (self.spaces.is_current(mesh), region(mesh, moved)) {
            (true, Some(region)) => {
                let fans = self.spaces.fans_around(mesh, &region);
                self.spaces.refresh(mesh, &fans);
                let unsettled = std::mem::take(&mut self.unsettled);
                let loops = region
                    .iter()
                    .flat_map(|&vert| mesh.vert_loops(vert))
                    .chain(unsettled);
                self.spaces.changed_at(mesh, &self.data, loops)
            }
            (current, _) => {
                match current {
                    true => self.spaces.refresh_all(mesh),
                    false => self.spaces = NormalSpaces::new(mesh),
                }
                self.unsettled.clear();
                self.spaces.changed_all(mesh, &self.data)
            }
        };
        mesh.set_custom_normals(&changed);
        self.spaces.synced = Synced::new(mesh);
    }
}

impl EditMesh {
    pub fn set_custom_normals(&mut self, changed: &[(LoopId, Vec3)]) {
        if changed.is_empty() {
            return;
        }
        let Some(values) = self
            .loop_data
            .attributes
            .values_mut::<Vec3>(CUSTOM_NORMAL, Domain::Corner)
        else {
            let values = self.custom_normals_mut();
            for &(lp, normal) in changed {
                values[lp.index()] = normal;
            }
            return;
        };
        for &(lp, normal) in changed {
            values[lp.index()] = normal;
        }
        match changed.len() * BULK_SHARE > self.loop_count() {
            true => self.forget_corners(),
            false => {
                for &(lp, _) in changed {
                    self.log_corner(lp);
                }
            }
        }
        self.touch_moved();
    }
}

#[must_use]
pub fn corner_normals(mesh: &EditMesh) -> Vec<Vec3> {
    NormalSpaces::new(mesh).corner_normals(mesh)
}
