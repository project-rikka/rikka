use std::cmp::Ordering;
use std::collections::BinaryHeap;

use bitflags::bitflags;
use glam::Vec3;
use rustc_hash::FxHashMap;

use crate::ids::{EdgeId, Element, FaceId, LoopId, VertId};
use crate::math::angle_normalized;
use crate::mesh::{EditMesh, Flags, SelectMode};
use crate::random::Rng;
use crate::select::indices_where;
use crate::sets::{IdMap, IdSet};
use crate::walk::{Walk, WalkDelimit};

const MIRROR_DISTANCE: f32 = 0.00002;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SelectAction {
    Select,
    Deselect,
    Invert,
    #[default]
    Toggle,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PickAction {
    #[default]
    Set,
    Extend,
    Deselect,
    Toggle,
}

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct LinkedDelimit: u8 {
        const SEAM = 1;
        const SHARP = 1 << 1;
        const MATERIAL = 1 << 2;
        const NORMAL = 1 << 3;
        const UV = 1 << 4;
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Axes: u8 {
        const X = 1;
        const Y = 1 << 1;
        const Z = 1 << 2;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NonManifold {
    pub extend: bool,
    pub wire: bool,
    pub boundary: bool,
    pub multi_face: bool,
    pub non_contiguous: bool,
    pub verts: bool,
}

impl Default for NonManifold {
    fn default() -> NonManifold {
        NonManifold {
            extend: true,
            wire: true,
            boundary: true,
            multi_face: true,
            non_contiguous: true,
            verts: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Checker {
    pub deselected: u32,
    pub selected: u32,
    pub offset: i32,
}

impl Default for Checker {
    fn default() -> Checker {
        Checker {
            deselected: 1,
            selected: 1,
            offset: 0,
        }
    }
}

impl Checker {
    fn deselects(&self, depth: usize) -> bool {
        !self.keeps(depth as i64)
    }

    pub(crate) fn keeps(&self, depth: i64) -> bool {
        let period = i64::from(self.selected + self.deselected).max(1);
        let offset = i64::from(self.offset).rem_euclid(period);
        self.deselected == 0 || (offset + depth).rem_euclid(period) >= i64::from(self.deselected)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Compare {
    #[default]
    Equal,
    NotEqual,
    Greater,
    Less,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct GroupCost {
    cost: f32,
    group: usize,
}

impl Eq for GroupCost {}

impl PartialOrd for GroupCost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for GroupCost {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cost
            .total_cmp(&other.cost)
            .then_with(|| other.group.cmp(&self.group))
    }
}

impl EditMesh {
    fn any_selected(&self) -> bool {
        self.verts().any(|vert| self.is_vert_selected(vert))
            || self.edges().any(|edge| self.is_edge_selected(edge))
            || self.faces().any(|face| self.is_face_selected(face))
    }

    pub fn select_all(&mut self, action: SelectAction) {
        let action = match action {
            SelectAction::Toggle => match self.any_selected() {
                true => SelectAction::Deselect,
                false => SelectAction::Select,
            },
            other => other,
        };
        match action {
            SelectAction::Select => {
                let unselected = |flags: Flags| !flags.intersects(Flags::SELECT | Flags::HIDDEN);
                let verts = indices_where(self.verts.capacity(), |index| {
                    self.verts.is_alive(index as u32) && unselected(self.verts.records[index].flags)
                });
                let edges = indices_where(self.edges.capacity(), |index| {
                    self.edges.is_alive(index as u32) && unselected(self.edges.records[index].flags)
                });
                let faces = indices_where(self.faces.capacity(), |index| {
                    self.faces.is_alive(index as u32) && unselected(self.faces.records[index].flags)
                });
                for index in verts {
                    self.mark_vert(VertId(index), Flags::SELECT, true);
                }
                for index in edges {
                    self.mark_edge(EdgeId(index), Flags::SELECT, true);
                }
                for index in faces {
                    self.mark_face(FaceId(index), Flags::SELECT, true);
                }
            }
            SelectAction::Deselect | SelectAction::Toggle => self.deselect_all(),
            SelectAction::Invert => {
                if self.select_mode.contains(SelectMode::VERT) {
                    for vert in self.verts().collect::<Vec<_>>() {
                        if !self.is_vert_hidden(vert) {
                            self.select_vert(vert, !self.is_vert_selected(vert));
                        }
                    }
                } else if self.select_mode.contains(SelectMode::EDGE) {
                    for edge in self.edges().collect::<Vec<_>>() {
                        if !self.is_edge_hidden(edge) {
                            self.select_edge(edge, !self.is_edge_selected(edge));
                        }
                    }
                } else {
                    for face in self.faces().collect::<Vec<_>>() {
                        if !self.is_face_hidden(face) {
                            self.select_face(face, !self.is_face_selected(face));
                        }
                    }
                }
                self.select_mode_flush();
            }
        }
        self.touch_selection();
    }

    pub fn deselect_all(&mut self) {
        let selected = |flags: Flags| flags.contains(Flags::SELECT);
        let verts = indices_where(self.verts.capacity(), |index| {
            selected(self.verts.records[index].flags)
        });
        let edges = indices_where(self.edges.capacity(), |index| {
            selected(self.edges.records[index].flags)
        });
        let faces = indices_where(self.faces.capacity(), |index| {
            selected(self.faces.records[index].flags)
        });
        for index in verts {
            self.mark_vert(VertId(index), Flags::SELECT, false);
        }
        for index in edges {
            self.mark_edge(EdgeId(index), Flags::SELECT, false);
        }
        for index in faces {
            self.mark_face(FaceId(index), Flags::SELECT, false);
        }
        self.history.clear();
        self.touch_selection();
    }

    fn face_select_flush_raw(&mut self, face: FaceId, marks: &mut Geometry3) {
        marks.faces.insert(face);
        for lp in self.face_loops(face) {
            marks.edges.insert(self.loop_edge(lp));
            marks.verts.insert(self.loop_vert(lp));
        }
    }

    fn region_extend(&mut self, contract: bool, face_step: bool) -> Geometry3 {
        let use_faces = self.select_mode == SelectMode::FACE;
        let mut out = Geometry3::default();
        if !use_faces {
            let original_edges: IdSet<EdgeId> = self
                .edges()
                .filter(|&edge| self.is_edge_selected(edge))
                .collect();
            let original_faces: IdSet<FaceId> = self
                .faces()
                .filter(|&face| self.is_face_selected(face))
                .collect();
            let seeds: Vec<VertId> = self
                .verts()
                .filter(|&vert| self.is_vert_selected(vert) && !self.is_vert_hidden(vert))
                .collect();
            for vert in seeds {
                if !contract {
                    let found = self
                        .vert_edges(vert)
                        .any(|edge| !original_edges.contains(edge) && !self.is_edge_hidden(edge));
                    if !found {
                        continue;
                    }
                    if !face_step {
                        for edge in self.vert_edges(vert).collect::<Vec<_>>() {
                            if !out.edges.contains(edge) && !self.is_edge_hidden(edge) {
                                out.edges.insert(edge);
                                out.verts.extend(self.edge_other_vert(edge, vert));
                            }
                        }
                    } else {
                        for face in self.vert_faces(vert).collect::<Vec<_>>() {
                            if !out.faces.contains(face) && !self.is_face_hidden(face) {
                                self.face_select_flush_raw(face, &mut out);
                            }
                        }
                        for edge in self.vert_edges(vert).collect::<Vec<_>>() {
                            if self.is_edge_wire(edge)
                                && !out.edges.contains(edge)
                                && !self.is_edge_hidden(edge)
                            {
                                out.edges.insert(edge);
                                out.verts.extend(self.edge_other_vert(edge, vert));
                            }
                        }
                    }
                } else {
                    let found = match face_step {
                        false => self
                            .vert_edges(vert)
                            .any(|edge| !original_edges.contains(edge)),
                        true => {
                            self.vert_faces(vert)
                                .any(|face| !original_faces.contains(face))
                                || self.vert_edges(vert).any(|edge| {
                                    self.is_edge_wire(edge) && !original_edges.contains(edge)
                                })
                        }
                    };
                    if found {
                        out.verts.insert(vert);
                        out.edges.extend(self.vert_edges(vert));
                    }
                }
            }
        } else {
            let original: IdSet<FaceId> = self
                .faces()
                .filter(|&face| self.is_face_selected(face))
                .collect();
            let seeds: Vec<FaceId> = original
                .iter()
                .filter(|&face| !self.is_face_hidden(face))
                .collect();
            for face in seeds {
                for lp in self.face_loops_vec(face) {
                    let neighbours: Vec<FaceId> = match face_step {
                        false => self.edge_faces(self.loop_edge(lp)).collect(),
                        true => self.vert_faces(self.loop_vert(lp)).collect(),
                    };
                    if !contract {
                        for other in neighbours {
                            if !original.contains(other)
                                && !out.faces.contains(other)
                                && !self.is_face_hidden(other)
                            {
                                out.faces.insert(other);
                            }
                        }
                    } else if neighbours.iter().any(|&other| !original.contains(other)) {
                        out.faces.insert(face);
                    }
                }
            }
        }
        out
    }

    pub fn select_more(&mut self, face_step: bool) {
        let grown = self.region_extend(false, face_step);
        self.apply_geometry_selection(&grown, true);
        self.select_mode_flush();
    }

    pub fn select_less(&mut self, face_step: bool) {
        let shrunk = self.region_extend(true, face_step);
        self.apply_geometry_selection(&shrunk, false);
        self.select_mode_flush();
    }

    fn apply_geometry_selection(&mut self, geometry: &Geometry3, selected: bool) {
        if self.select_mode == SelectMode::FACE {
            for vert in geometry.verts.iter() {
                self.select_vert(vert, selected);
            }
            for edge in geometry.edges.iter() {
                self.select_edge(edge, selected);
            }
            for face in geometry.faces.iter() {
                self.select_face(face, selected);
            }
            return;
        }
        for vert in geometry.verts.iter() {
            self.set_vert_selected_raw(vert, selected);
        }
        for edge in geometry.edges.iter() {
            if !self.is_edge_hidden(edge) {
                self.mark_edge(edge, Flags::SELECT, selected);
            }
        }
        for face in geometry.faces.iter() {
            if !self.is_face_hidden(face) {
                self.mark_face(face, Flags::SELECT, selected);
            }
        }
        self.touch_selection();
    }

    pub(crate) fn edge_delimits(
        &self,
        edge: EdgeId,
        delimit: LinkedDelimit,
        uvs: Option<&str>,
    ) -> bool {
        if delimit.contains(LinkedDelimit::SEAM) && self.is_edge_seam(edge) {
            return true;
        }
        if delimit.contains(LinkedDelimit::SHARP) && self.is_edge_sharp(edge) {
            return true;
        }
        if delimit.contains(LinkedDelimit::NORMAL) && !self.is_edge_contiguous(edge) {
            return true;
        }
        if delimit.contains(LinkedDelimit::MATERIAL)
            && let Some(first) = self.edge_first_loop(edge)
        {
            let material = self.face_material(self.loop_face(first));
            if self
                .edge_loops(edge)
                .any(|lp| self.face_material(self.loop_face(lp)) != material)
            {
                return true;
            }
        }
        if let Some(uv_map) = uvs
            && delimit.contains(LinkedDelimit::UV)
            && !self.is_edge_uv_contiguous(edge, uv_map)
        {
            return true;
        }
        false
    }

    fn linked_region(&self, seeds: LinkedSeeds, delimit: LinkedDelimit) -> Geometry3 {
        let uv_map = self.layout.uv_maps.active_name().map(str::to_owned);
        let walkable: IdSet<EdgeId> = match delimit.is_empty() {
            true => IdSet::new(),
            false => self
                .edges()
                .filter(|&edge| !self.edge_delimits(edge, delimit, uv_map.as_deref()))
                .collect(),
        };
        let edge_ok = |edge: EdgeId| {
            !self.is_edge_hidden(edge) && (delimit.is_empty() || walkable.contains(edge))
        };
        let mut out = Geometry3::default();
        let mut face_queue: Vec<FaceId> = Vec::new();
        let mut vert_queue: Vec<VertId> = Vec::new();
        match seeds {
            LinkedSeeds::Faces(faces) => face_queue.extend(faces),
            LinkedSeeds::Verts(verts) => vert_queue.extend(verts),
        }
        loop {
            if let Some(face) = face_queue.pop() {
                if self.is_face_hidden(face) || !out.faces.insert(face) {
                    continue;
                }
                for lp in self.face_loops(face) {
                    let edge = self.loop_edge(lp);
                    out.verts.insert(self.loop_vert(lp));
                    out.edges.insert(edge);
                    if edge_ok(edge) {
                        face_queue.extend(
                            self.edge_faces(edge)
                                .filter(|other| !out.faces.contains(*other)),
                        );
                    }
                    if delimit.is_empty() {
                        vert_queue.push(self.loop_vert(lp));
                    }
                }
                continue;
            }
            let Some(vert) = vert_queue.pop() else {
                break;
            };
            if self.is_vert_hidden(vert) || out.visited_verts.contains(vert) {
                continue;
            }
            out.visited_verts.insert(vert);
            out.verts.insert(vert);
            for edge in self.vert_edges(vert) {
                if self.is_edge_hidden(edge) {
                    continue;
                }
                let wire = self.is_edge_wire(edge);
                if delimit.is_empty() || wire {
                    out.edges.insert(edge);
                    vert_queue.extend(self.edge_other_vert(edge, vert));
                }
                if !delimit.is_empty() {
                    face_queue.extend(
                        self.edge_faces(edge)
                            .filter(|face| !out.faces.contains(*face)),
                    );
                }
            }
        }
        out
    }

    pub fn select_linked(&mut self, delimit: LinkedDelimit) {
        let mode = self.select_mode;
        if mode.contains(SelectMode::VERT) || mode.contains(SelectMode::EDGE) {
            let seeds: Vec<VertId> = match mode.contains(SelectMode::VERT) {
                true => self
                    .verts()
                    .filter(|&vert| self.is_vert_selected(vert))
                    .collect(),
                false => self
                    .edges()
                    .filter(|&edge| self.is_edge_selected(edge))
                    .flat_map(|edge| self.edge_verts(edge))
                    .collect(),
            };
            let region = self.linked_region(LinkedSeeds::Verts(seeds), delimit);
            for edge in region.edges.iter() {
                self.select_edge(edge, true);
            }
            for vert in region.verts.iter() {
                self.select_vert(vert, true);
            }
            self.select_mode_flush();
        } else {
            let seeds: Vec<FaceId> = self
                .faces()
                .filter(|&face| self.is_face_selected(face))
                .collect();
            let region = self.linked_region(LinkedSeeds::Faces(seeds), delimit);
            for face in region.faces.iter() {
                self.select_face(face, true);
            }
        }
        self.touch_selection();
    }

    pub fn select_linked_pick(&mut self, element: Element, select: bool, delimit: LinkedDelimit) {
        let region = match element {
            Element::Vert(vert) => self.linked_region(LinkedSeeds::Verts(vec![vert]), delimit),
            Element::Edge(edge) => {
                self.linked_region(LinkedSeeds::Verts(self.edge_verts(edge).to_vec()), delimit)
            }
            Element::Face(face) => self.linked_region(LinkedSeeds::Faces(vec![face]), delimit),
        };
        if self.select_mode.contains(SelectMode::FACE)
            && !self
                .select_mode
                .intersects(SelectMode::VERT | SelectMode::EDGE)
        {
            let faces: Vec<FaceId> = match element {
                Element::Face(_) => region.faces.to_vec(),
                _ => region.faces.to_vec(),
            };
            for face in faces {
                self.select_face(face, select);
            }
        } else {
            for edge in region.edges.iter() {
                self.select_edge(edge, select);
            }
            for vert in region.verts.iter() {
                self.select_vert(vert, select);
            }
        }
        if !select {
            self.history_validate();
        }
        self.select_mode_flush();
    }

    fn walker_select<I: Copy>(
        &mut self,
        elements: &[I],
        select: bool,
        apply: impl Fn(&mut EditMesh, I, bool) -> Element,
    ) {
        for &element in elements {
            let element = apply(self, element, select);
            if !select {
                self.history_remove(element);
            }
        }
    }

    fn walk_count(&self, elements: &[EdgeId]) -> [usize; 2] {
        let mut counts = [0usize; 2];
        for &edge in elements {
            counts[usize::from(self.is_edge_selected(edge))] += 1;
        }
        counts
    }

    pub fn pick_loop(
        &mut self,
        edge: EdgeId,
        ring: bool,
        action: PickAction,
        delimit: WalkDelimit,
    ) {
        let select = match action {
            PickAction::Set | PickAction::Extend => true,
            PickAction::Deselect => false,
            PickAction::Toggle => !self.is_edge_selected(edge),
        };
        let clear = action == PickAction::Set;
        let walk = Walk::visible().delimit(delimit);
        if self.select_mode.contains(SelectMode::FACE)
            && !self
                .select_mode
                .intersects(SelectMode::VERT | SelectMode::EDGE)
        {
            if clear {
                self.deselect_all();
            }
            let faces = self.walk_face_loop(edge, &walk);
            self.walker_select(&faces, select, |mesh, face, select| {
                mesh.select_face(face, select);
                Element::Face(face)
            });
        } else if ring {
            let mut full = false;
            if delimit.intersects(WalkDelimit::SEAM | WalkDelimit::SHARP) {
                let counts = self.walk_count(&self.walk_edge_ring(edge, &walk));
                if counts[usize::from(!select)] == 0 {
                    full = true;
                    let all = self.walk_count(&self.walk_edge_ring(edge, &Walk::visible()));
                    if all[usize::from(!select)] == 0 {
                        full = false;
                    }
                }
            }
            if clear {
                self.deselect_all();
            }
            let edges = match full {
                true => self.walk_edge_ring(edge, &Walk::visible()),
                false => self.walk_edge_ring(edge, &walk),
            };
            self.walker_select(&edges, select, |mesh, edge, select| {
                mesh.select_edge(edge, select);
                Element::Edge(edge)
            });
        } else {
            let non_manifold = self.edge_face_count_at_most(edge, 3) > 2;
            let mut full_boundary = false;
            let mut full_loop = false;
            if self.is_edge_boundary(edge) {
                let counts = self.walk_count(&self.walk_edge_loop(edge, &walk));
                if counts[usize::from(!select)] == 0 {
                    full_boundary = true;
                    let boundary = self.walk_count(&self.walk_boundary(edge, &Walk::visible()));
                    if boundary[usize::from(!select)] == 0 {
                        full_boundary = false;
                    }
                }
            } else if !non_manifold && delimit.intersects(WalkDelimit::SEAM | WalkDelimit::SHARP) {
                let counts = self.walk_count(&self.walk_edge_loop(edge, &walk));
                if counts[usize::from(!select)] == 0 {
                    full_loop = true;
                    let all = self.walk_count(&self.walk_edge_loop(edge, &Walk::visible()));
                    if all[usize::from(!select)] == 0 {
                        full_loop = false;
                    }
                }
            }
            if clear {
                self.deselect_all();
            }
            let edges = if full_boundary {
                self.walk_boundary(edge, &Walk::visible())
            } else if non_manifold {
                self.walk_non_manifold_loop(edge)
            } else if full_loop {
                self.walk_edge_loop(edge, &Walk::visible())
            } else {
                self.walk_edge_loop(edge, &walk)
            };
            self.walker_select(&edges, select, |mesh, edge, select| {
                mesh.select_edge(edge, select);
                Element::Edge(edge)
            });
        }
        self.select_mode_flush();
        if select && self.select_mode.contains(SelectMode::EDGE) {
            self.history_store(Element::Edge(edge));
        }
    }

    pub fn select_edge_loops(&mut self, delimit: WalkDelimit) -> bool {
        let selected: Vec<EdgeId> = self
            .edges()
            .filter(|&edge| self.is_edge_selected(edge))
            .collect();
        let mut changed = false;
        for edge in selected {
            let edges = match self.edge_face_count_at_most(edge, 3) > 2 {
                true => self.walk_non_manifold_loop(edge),
                false => self.walk_edge_loop(edge, &Walk::visible().delimit(delimit)),
            };
            for walked in edges {
                self.select_edge(walked, true);
                changed = true;
            }
        }
        if changed {
            self.select_mode_flush();
        }
        changed
    }

    pub fn select_edge_rings(&mut self, delimit: WalkDelimit) -> bool {
        let selected: Vec<EdgeId> = self
            .edges()
            .filter(|&edge| self.is_edge_selected(edge))
            .collect();
        let mut changed = false;
        for edge in selected {
            for walked in self.walk_edge_ring(edge, &Walk::visible().delimit(delimit)) {
                self.select_edge(walked, true);
                changed = true;
            }
        }
        if changed {
            self.select_mode_flush();
        }
        changed
    }

    pub fn select_boundary_loops(&mut self, extend: bool, delimit: WalkDelimit) -> bool {
        let boundary: Vec<EdgeId> = self
            .edges()
            .filter(|&edge| self.is_edge_selected(edge) && self.is_edge_boundary(edge))
            .collect();
        if boundary.is_empty() {
            return false;
        }
        if !extend {
            self.deselect_all();
        }
        let mut done: IdSet<EdgeId> = IdSet::new();
        for edge in boundary {
            if done.contains(edge) {
                continue;
            }
            for walked in self.walk_edge_loop(edge, &Walk::visible().delimit(delimit)) {
                self.select_edge(walked, true);
                done.insert(walked);
            }
        }
        self.select_mode_flush();
        true
    }

    pub fn select_region_boundary(&mut self) {
        let mut boundary: Vec<EdgeId> = Vec::new();
        for face in self.faces().collect::<Vec<_>>() {
            if !self.is_face_selected(face) || self.is_face_hidden(face) {
                continue;
            }
            for lp in self.face_loops(face) {
                let edge = self.loop_edge(lp);
                let mut outside = false;
                let mut inside = 0;
                for other in self.edge_faces(edge) {
                    if other == face || self.is_face_hidden(other) {
                        continue;
                    }
                    match self.is_face_selected(other) {
                        true => inside += 1,
                        false => outside = true,
                    }
                }
                let alone = self
                    .edge_faces(edge)
                    .filter(|&other| !self.is_face_hidden(other))
                    .count()
                    == 1;
                if outside || alone || (inside == 0 && self.is_edge_boundary(edge)) {
                    boundary.push(edge);
                }
            }
        }
        let faces: Vec<FaceId> = self.faces().collect();
        for face in faces {
            self.mark_face(face, Flags::SELECT, false);
        }
        let edges: Vec<EdgeId> = self.edges().collect();
        for edge in edges {
            self.mark_edge(edge, Flags::SELECT, false);
        }
        let verts: Vec<VertId> = self.verts().collect();
        for vert in verts {
            self.mark_vert(vert, Flags::SELECT, false);
        }
        for edge in boundary {
            self.select_edge(edge, true);
        }
        if self.select_mode == SelectMode::FACE {
            self.set_select_mode(SelectMode::EDGE);
        } else {
            self.select_mode_flush();
        }
    }

    pub fn select_loop_inner_region(&mut self, select_bigger: bool) {
        let boundary: IdSet<EdgeId> = self
            .edges()
            .filter(|&edge| self.is_edge_selected(edge))
            .collect();
        let mut taken: IdSet<FaceId> = IdSet::new();
        let mut chosen: Vec<Vec<FaceId>> = Vec::new();
        let candidates: Vec<EdgeId> = boundary.iter().collect();
        for edge in candidates {
            for lp in self.edge_loops(edge).collect::<Vec<_>>() {
                let face = self.loop_face(lp);
                if taken.contains(face) || self.is_face_hidden(face) {
                    continue;
                }
                let region_a = self.region_bounded_by(face, &boundary);
                let other = self
                    .edge_loops(edge)
                    .map(|other| self.loop_face(other))
                    .find(|&other| other != face && !region_a.contains(&other));
                let region = match other {
                    Some(other) => {
                        let region_b = self.region_bounded_by(other, &boundary);
                        let a_smaller = region_a.len() < region_b.len();
                        match (a_smaller, select_bigger) {
                            (true, false) | (false, true) => region_a,
                            _ => region_b,
                        }
                    }
                    None => region_a,
                };
                for &face in &region {
                    taken.insert(face);
                }
                chosen.push(region);
                break;
            }
        }
        self.deselect_all();
        for region in chosen {
            for face in region {
                self.select_face(face, true);
            }
        }
        if !self.select_mode.contains(SelectMode::FACE) {
            self.select_mode_flush();
        }
    }

    fn region_bounded_by(&self, start: FaceId, boundary: &IdSet<EdgeId>) -> Vec<FaceId> {
        let mut seen: IdSet<FaceId> = IdSet::new();
        let mut order = Vec::new();
        let mut pending = vec![start];
        seen.insert(start);
        while let Some(face) = pending.pop() {
            order.push(face);
            for lp in self.face_loops(face) {
                let edge = self.loop_edge(lp);
                if boundary.contains(edge) {
                    continue;
                }
                for other in self.edge_faces(edge) {
                    if !self.is_face_hidden(other) && seen.insert(other) {
                        pending.push(other);
                    }
                }
            }
        }
        order
    }

    pub fn select_random(&mut self, ratio: f32, seed: u32, select: bool) {
        let mut random = Rng::new(seed);
        if self.select_mode.contains(SelectMode::VERT) {
            let mut verts: Vec<VertId> = self
                .verts()
                .filter(|&vert| !self.is_vert_hidden(vert))
                .collect();
            random.shuffle(&mut verts);
            let count = (verts.len() as f32 * ratio) as usize;
            for &vert in &verts[..count.min(verts.len())] {
                self.select_vert(vert, select);
            }
        } else if self.select_mode.contains(SelectMode::EDGE) {
            let mut edges: Vec<EdgeId> = self
                .edges()
                .filter(|&edge| !self.is_edge_hidden(edge))
                .collect();
            random.shuffle(&mut edges);
            let count = (edges.len() as f32 * ratio) as usize;
            for &edge in &edges[..count.min(edges.len())] {
                self.select_edge(edge, select);
            }
        } else {
            let mut faces: Vec<FaceId> = self
                .faces()
                .filter(|&face| !self.is_face_hidden(face))
                .collect();
            random.shuffle(&mut faces);
            let count = (faces.len() as f32 * ratio) as usize;
            for &face in &faces[..count.min(faces.len())] {
                self.select_face(face, select);
            }
        }
        match select {
            true => self.select_mode_flush(),
            false => self.select_flush_from_verts(false),
        }
    }

    fn checker_start(&mut self) -> Option<Element> {
        self.select_mode_flush();
        if let Some(active) = self.active_element()
            && self.is_element_selected(active)
        {
            return Some(active);
        }
        if self.select_mode.contains(SelectMode::VERT) {
            return self
                .verts()
                .find(|&vert| self.is_vert_selected(vert))
                .map(Element::Vert);
        }
        if self.select_mode.contains(SelectMode::EDGE) {
            return self
                .edges()
                .find(|&edge| self.is_edge_selected(edge))
                .map(Element::Edge);
        }
        self.active_face()
            .filter(|&face| self.is_face_selected(face))
            .or_else(|| self.faces().find(|&face| self.is_face_selected(face)))
            .map(Element::Face)
    }

    pub fn deselect_checker(&mut self, checker: Checker) -> bool {
        let Some(start) = self.checker_start() else {
            return false;
        };
        match start {
            Element::Vert(vert) => {
                if let Some(chain) = self.selected_vert_cycle(vert) {
                    for (index, vert) in chain.into_iter().enumerate() {
                        if checker.deselects(index) {
                            self.select_vert(vert, false);
                        }
                    }
                } else {
                    let depths = self.breadth_depths(vert, |mesh, vert| {
                        mesh.vert_verts(vert)
                            .filter(|&other| mesh.is_vert_selected(other))
                            .collect()
                    });
                    for (vert, depth) in depths {
                        if checker.deselects(depth) {
                            self.select_vert(vert, false);
                        }
                    }
                }
                self.select_mode_flush_with(SelectMode::VERT, false);
            }
            Element::Edge(edge) => {
                if let Some(chain) = self.selected_edge_cycle(edge) {
                    for (index, edge) in chain.into_iter().enumerate() {
                        if checker.deselects(index) {
                            self.select_edge(edge, false);
                        }
                    }
                } else {
                    let isolated = self.edge_verts(edge).iter().all(|&vert| {
                        self.vert_edges(vert)
                            .all(|other| other == edge || !self.is_edge_selected(other))
                    });
                    let depths = self.breadth_depths(edge, |mesh, edge| match isolated {
                        true => mesh
                            .edge_loops(edge)
                            .flat_map(|lp| mesh.face_edges(mesh.loop_face(lp)).collect::<Vec<_>>())
                            .filter(|&other| mesh.is_edge_selected(other))
                            .collect(),
                        false => mesh
                            .edge_verts(edge)
                            .iter()
                            .flat_map(|&vert| mesh.vert_edges(vert).collect::<Vec<_>>())
                            .filter(|&other| mesh.is_edge_selected(other))
                            .collect(),
                    });
                    for (edge, depth) in depths {
                        if checker.deselects(depth) {
                            self.select_edge(edge, false);
                        }
                    }
                }
                self.select_mode_flush_with(SelectMode::EDGE, false);
            }
            Element::Face(face) => {
                if let Some(chain) = self.selected_face_cycle(face) {
                    for (index, face) in chain.into_iter().enumerate() {
                        if checker.deselects(index) {
                            self.select_face(face, false);
                        }
                    }
                } else {
                    let depths = self.breadth_depths(face, |mesh, face| {
                        mesh.face_loops(face)
                            .flat_map(|lp| mesh.edge_faces(mesh.loop_edge(lp)).collect::<Vec<_>>())
                            .filter(|&other| other != face && mesh.is_face_selected(other))
                            .collect()
                    });
                    for (face, depth) in depths {
                        if checker.deselects(depth) {
                            self.select_face(face, false);
                        }
                    }
                }
                self.select_mode_flush_with(SelectMode::FACE, false);
            }
        }
        true
    }

    fn breadth_depths<I: crate::ElementId>(
        &self,
        start: I,
        neighbours: impl Fn(&EditMesh, I) -> Vec<I>,
    ) -> Vec<(I, usize)> {
        let mut seen: IdSet<I> = IdSet::new();
        let mut out = vec![(start, 0)];
        seen.insert(start);
        let mut index = 0;
        while index < out.len() {
            let (element, depth) = out[index];
            for other in neighbours(self, element) {
                if seen.insert(other) {
                    out.push((other, depth + 1));
                }
            }
            index += 1;
        }
        out
    }

    fn selected_vert_cycle(&self, start: VertId) -> Option<Vec<VertId>> {
        let selected_neighbours = |vert: VertId, previous: Option<VertId>| -> Vec<VertId> {
            self.vert_verts(vert)
                .filter(|&other| Some(other) != previous && self.is_vert_selected(other))
                .collect()
        };
        let mut chain = vec![start];
        let mut previous = None;
        let mut current = start;
        loop {
            if selected_neighbours(current, None).len() != 2 {
                return None;
            }
            let next = selected_neighbours(current, previous);
            let expected = if previous.is_some() { 1 } else { 2 };
            if next.len() != expected {
                return None;
            }
            let next = next[0];
            if next == start {
                return Some(chain);
            }
            if chain.contains(&next) {
                return None;
            }
            chain.push(next);
            previous = Some(current);
            current = next;
        }
    }

    fn selected_edge_cycle(&self, start: EdgeId) -> Option<Vec<EdgeId>> {
        let mut chain = vec![start];
        let mut current = start;
        let mut through = self.edge_verts(start)[0];
        loop {
            let next: Vec<EdgeId> = self
                .vert_edges(through)
                .filter(|&other| other != current && self.is_edge_selected(other))
                .collect();
            if next.len() != 1 {
                return None;
            }
            let next = next[0];
            if next == start {
                return Some(chain);
            }
            if chain.contains(&next) {
                return None;
            }
            through = self.edge_other_vert(next, through)?;
            chain.push(next);
            current = next;
        }
    }

    fn selected_face_cycle(&self, start: FaceId) -> Option<Vec<FaceId>> {
        let neighbours = |face: FaceId, previous: Option<FaceId>| -> Vec<FaceId> {
            self.face_loops(face)
                .flat_map(|lp| self.edge_faces(self.loop_edge(lp)).collect::<Vec<_>>())
                .filter(|&other| {
                    other != face && Some(other) != previous && self.is_face_selected(other)
                })
                .collect()
        };
        let mut chain = vec![start];
        let mut previous = None;
        let mut current = start;
        loop {
            if neighbours(current, None).len() != 2 {
                return None;
            }
            let next = neighbours(current, previous);
            let expected = if previous.is_some() { 1 } else { 2 };
            if next.len() != expected {
                return None;
            }
            let next = next[0];
            if next == start {
                return Some(chain);
            }
            if chain.contains(&next) {
                return None;
            }
            chain.push(next);
            previous = Some(current);
            current = next;
        }
    }

    pub fn select_non_manifold(&mut self, options: NonManifold) -> bool {
        if self.select_mode == SelectMode::FACE {
            return false;
        }
        if !options.extend {
            self.deselect_all();
        }
        if options.verts && self.select_mode.contains(SelectMode::VERT) {
            for vert in self.verts().collect::<Vec<_>>() {
                if !self.is_vert_hidden(vert) && !self.is_vert_manifold(vert) {
                    self.select_vert(vert, true);
                }
            }
        }
        if options.wire || options.boundary || options.multi_face || options.non_contiguous {
            for edge in self.edges().collect::<Vec<_>>() {
                if self.is_edge_hidden(edge) {
                    continue;
                }
                let hit = (options.wire && self.is_edge_wire(edge))
                    || (options.boundary && self.is_edge_boundary(edge))
                    || (options.non_contiguous
                        && self.is_edge_manifold(edge)
                        && !self.is_edge_contiguous(edge))
                    || (options.multi_face && self.edge_face_count_at_most(edge, 3) > 2);
                if hit {
                    self.select_edge(edge, true);
                }
            }
        }
        self.select_mode_flush();
        true
    }

    pub fn select_loose(&mut self, extend: bool) {
        if !extend {
            self.deselect_all();
        }
        if self.select_mode.contains(SelectMode::VERT) {
            for vert in self.verts().collect::<Vec<_>>() {
                if self.is_vert_loose(vert) {
                    self.select_vert(vert, true);
                }
            }
        }
        if self.select_mode.contains(SelectMode::EDGE) {
            for edge in self.edges().collect::<Vec<_>>() {
                if self.is_edge_wire(edge) {
                    self.select_edge(edge, true);
                }
            }
        }
        if self.select_mode.contains(SelectMode::FACE) {
            for face in self.faces().collect::<Vec<_>>() {
                if self
                    .face_edges(face)
                    .all(|edge| self.is_edge_boundary(edge))
                {
                    self.select_face(face, true);
                }
            }
        }
        self.select_mode_flush();
    }

    pub fn select_interior_faces(&mut self) -> bool {
        let tagged: IdSet<EdgeId> = self
            .edges()
            .filter(|&edge| self.edge_face_count_at_most(edge, 3) > 2)
            .collect();
        if tagged.is_empty() {
            return false;
        }
        let mut tagged = tagged;
        let mut group_of: crate::IdMap<FaceId, usize> = crate::IdMap::new();
        let mut groups: Vec<Vec<(FaceId, f32)>> = Vec::new();
        for face in self.faces() {
            if group_of.contains(face) {
                continue;
            }
            let group = groups.len();
            let mut members = Vec::new();
            let mut pending = vec![face];
            group_of.insert(face, group);
            while let Some(current) = pending.pop() {
                members.push((current, self.face_area(current)));
                for lp in self.face_loops(current) {
                    let edge = self.loop_edge(lp);
                    if tagged.contains(edge) {
                        continue;
                    }
                    for other in self.edge_faces(edge) {
                        if !group_of.contains(other) {
                            group_of.insert(other, group);
                            pending.push(other);
                        }
                    }
                }
            }
            groups.push(members);
        }
        let mut membership: crate::IdMap<FaceId, Option<usize>> = group_of
            .iter()
            .map(|(face, &group)| (face, Some(group)))
            .collect();
        let cost = |mesh: &EditMesh,
                    members: &[(FaceId, f32)],
                    membership: &crate::IdMap<FaceId, Option<usize>>,
                    tagged: &IdSet<EdgeId>|
         -> Option<f32> {
            let mut area = 0.0;
            let mut total = 0.0;
            let mut found = false;
            for &(face, face_area) in members {
                area += face_area;
                let own = membership.copied(face).flatten();
                for lp in mesh.face_loops(face) {
                    let edge = mesh.loop_edge(lp);
                    if !tagged.contains(edge) {
                        continue;
                    }
                    let mut edge_cost = 0.0;
                    let mut count = 0;
                    for other in mesh.edge_faces(edge) {
                        let other_group = membership.copied(other).flatten();
                        if other_group.is_some() && other_group != own {
                            let mut angle =
                                angle_normalized(mesh.face_normal(face), mesh.face_normal(other));
                            if angle > std::f32::consts::FRAC_PI_2 {
                                angle = std::f32::consts::PI - angle;
                            }
                            edge_cost += mesh.edge_length(edge) * angle;
                            count += 1;
                        }
                    }
                    if count >= 2 {
                        total += edge_cost;
                        found = true;
                    }
                }
            }
            found.then_some(total / area)
        };
        let mut alive = vec![true; groups.len()];
        let mut versions = vec![0u32; groups.len()];
        let mut heap: BinaryHeap<(GroupCost, u32)> = BinaryHeap::new();
        for (group, members) in groups.iter().enumerate() {
            match cost(self, members, &membership, &tagged) {
                Some(value) => heap.push((GroupCost { cost: value, group }, 0)),
                None => alive[group] = false,
            }
        }
        let mut changed = false;
        while let Some((entry, version)) = heap.pop() {
            let group = entry.group;
            if !alive[group] || version != versions[group] {
                continue;
            }
            alive[group] = false;
            changed = true;
            let members = std::mem::take(&mut groups[group]);
            let mut dirty: Vec<usize> = Vec::new();
            for &(face, _) in &members {
                self.select_face(face, true);
                membership.insert(face, None);
            }
            for &(face, _) in &members {
                for lp in self.face_loops_vec(face) {
                    let edge = self.loop_edge(lp);
                    let others: Vec<FaceId> = self
                        .edge_faces(edge)
                        .filter(|&other| {
                            membership
                                .copied(other)
                                .flatten()
                                .is_some_and(|g| g != group)
                        })
                        .collect();
                    if others.len() == 2 {
                        tagged.remove(edge);
                        let (mut a, mut b) = (
                            membership.copied(others[0]).flatten().unwrap_or(group),
                            membership.copied(others[1]).flatten().unwrap_or(group),
                        );
                        if a != b {
                            if a > b {
                                std::mem::swap(&mut a, &mut b);
                            }
                            let moved = std::mem::take(&mut groups[b]);
                            for &(member, _) in &moved {
                                membership.insert(member, Some(a));
                            }
                            groups[a].extend(moved);
                            alive[b] = false;
                            dirty.push(a);
                        }
                    }
                    for other in self.edge_faces(edge).collect::<Vec<_>>() {
                        if let Some(other_group) = membership.copied(other).flatten()
                            && other_group != group
                            && alive[other_group]
                        {
                            dirty.push(other_group);
                        }
                    }
                }
            }
            dirty.sort_unstable();
            dirty.dedup();
            for group in dirty {
                if !alive[group] {
                    continue;
                }
                versions[group] += 1;
                match cost(self, &groups[group], &membership, &tagged) {
                    Some(value) => heap.push((GroupCost { cost: value, group }, versions[group])),
                    None => alive[group] = false,
                }
            }
        }
        changed
    }

    pub fn select_faces_by_sides(&mut self, sides: usize, compare: Compare, extend: bool) {
        if !extend {
            self.deselect_all();
        }
        for face in self.faces().collect::<Vec<_>>() {
            let len = self.face_len(face);
            let hit = match compare {
                Compare::Equal => len == sides,
                Compare::NotEqual => len != sides,
                Compare::Greater => len > sides,
                Compare::Less => len < sides,
            };
            if hit {
                self.select_face(face, true);
            }
        }
        self.select_mode_flush();
    }

    pub fn select_sharp_edges(&mut self, angle: f32) {
        for edge in self.edges().collect::<Vec<_>>() {
            if self.is_edge_hidden(edge) {
                continue;
            }
            let Some((a, b)) = self.edge_loop_pair(edge) else {
                continue;
            };
            let face_a = self.loop_face(a);
            let face_b = self.loop_face(b);
            if self.is_face_hidden(face_a) || self.is_face_hidden(face_b) {
                continue;
            }
            if angle_normalized(self.face_normal(face_a), self.face_normal(face_b)) > angle {
                self.select_edge(edge, true);
            }
        }
        self.select_mode_flush();
    }

    pub fn select_linked_flat_faces(&mut self, sharpness: f32) {
        let threshold = sharpness.cos();
        let mut visited: IdSet<FaceId> = IdSet::new();
        let seeds: Vec<FaceId> = self
            .faces()
            .filter(|&face| self.is_face_selected(face))
            .collect();
        for seed in seeds {
            if self.is_face_hidden(seed) || visited.contains(seed) {
                continue;
            }
            let mut pending = vec![seed];
            visited.insert(seed);
            while let Some(face) = pending.pop() {
                self.select_face(face, true);
                for lp in self.face_loops_vec(face) {
                    let edge = self.loop_edge(lp);
                    let Some(other_lp) = self
                        .edge_first_loop(edge)
                        .map(|_| self.loop_radial_next(lp))
                    else {
                        continue;
                    };
                    if other_lp == lp || !self.is_edge_manifold(edge) {
                        continue;
                    }
                    let other = self.loop_face(other_lp);
                    if visited.contains(other) || self.is_face_hidden(other) {
                        continue;
                    }
                    if self.face_normal(face).dot(self.face_normal(other)) > threshold {
                        visited.insert(other);
                        pending.push(other);
                    }
                }
            }
        }
        self.select_mode_flush();
    }

    fn mirror_lookup(&self, axis: usize) -> impl Fn(VertId) -> Option<VertId> + '_ {
        let cell =
            |point: Vec3| -> [i64; 3] { (point / MIRROR_DISTANCE).floor().as_i64vec3().to_array() };
        let mut grid: FxHashMap<[i64; 3], Vec<VertId>> = FxHashMap::default();
        for vert in self.verts() {
            grid.entry(cell(self.position(vert)))
                .or_default()
                .push(vert);
        }
        move |vert: VertId| {
            let mut mirrored = self.position(vert);
            mirrored[axis] = -mirrored[axis];
            let center = cell(mirrored);
            let mut best: Option<(f32, VertId)> = None;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let key = [center[0] + dx, center[1] + dy, center[2] + dz];
                        for &other in grid.get(&key).into_iter().flatten() {
                            let distance = self.position(other).distance(mirrored);
                            if distance <= MIRROR_DISTANCE
                                && best.is_none_or(|(best, id)| {
                                    distance < best || (distance == best && other < id)
                                })
                            {
                                best = Some((distance, other));
                            }
                        }
                    }
                }
            }
            best.map(|(_, other)| other)
        }
    }

    pub fn select_mirror(&mut self, axes: Axes, extend: bool) -> (usize, usize) {
        self.select_mirror_mapped(axes, extend, None)
    }

    pub fn select_mirror_mapped(
        &mut self,
        axes: Axes,
        extend: bool,
        mirror: Option<&IdMap<VertId, VertId>>,
    ) -> (usize, usize) {
        let mut mirrored = 0;
        let mut failed = 0;
        for (axis, flag) in [(0, Axes::X), (1, Axes::Y), (2, Axes::Z)] {
            if !axes.contains(flag) {
                continue;
            }
            let mode = self.select_mode;
            let tagged_verts: Vec<VertId> = self
                .verts()
                .filter(|&vert| self.is_vert_selected(vert))
                .collect();
            let tagged_edges: Vec<EdgeId> = self
                .edges()
                .filter(|&edge| self.is_edge_selected(edge))
                .collect();
            let tagged_faces: Vec<FaceId> = self
                .faces()
                .filter(|&face| self.is_face_selected(face))
                .collect();
            let (verts, edges, faces) = {
                let spatial = mirror.is_none().then(|| self.mirror_lookup(axis));
                let lookup = |vert: VertId| match (mirror, &spatial) {
                    (Some(map), _) => map.copied(vert),
                    (None, Some(spatial)) => spatial(vert),
                    (None, None) => None,
                };
                let verts: Vec<Option<VertId>> =
                    tagged_verts.iter().map(|&vert| lookup(vert)).collect();
                let edges: Vec<Option<EdgeId>> = tagged_edges
                    .iter()
                    .map(|&edge| {
                        let [a, b] = self.edge_verts(edge);
                        self.edge_between(lookup(a)?, lookup(b)?)
                    })
                    .collect();
                let faces: Vec<Option<FaceId>> = tagged_faces
                    .iter()
                    .map(|&face| {
                        let verts: Option<Vec<VertId>> =
                            self.face_verts(face).map(&lookup).collect();
                        self.face_exists(&verts?)
                    })
                    .collect();
                (verts, edges, faces)
            };
            if !extend {
                self.deselect_all();
            }
            if mode.contains(SelectMode::VERT) {
                for (&vert, target) in tagged_verts.iter().zip(verts) {
                    if self.is_vert_hidden(vert) {
                        continue;
                    }
                    match target.filter(|&target| !self.is_vert_hidden(target)) {
                        Some(target) => {
                            self.select_vert(target, true);
                            mirrored += 1;
                        }
                        None => failed += 1,
                    }
                }
            } else if mode.contains(SelectMode::EDGE) {
                for (&edge, target) in tagged_edges.iter().zip(edges) {
                    if self.is_edge_hidden(edge) {
                        continue;
                    }
                    match target.filter(|&target| !self.is_edge_hidden(target)) {
                        Some(target) => {
                            self.select_edge(target, true);
                            mirrored += 1;
                        }
                        None => failed += 1,
                    }
                }
            } else {
                for (&face, target) in tagged_faces.iter().zip(faces) {
                    if self.is_face_hidden(face) {
                        continue;
                    }
                    match target.filter(|&target| !self.is_face_hidden(target)) {
                        Some(target) => {
                            self.select_face(target, true);
                            mirrored += 1;
                        }
                        None => failed += 1,
                    }
                }
            }
            if mirrored > 0 {
                self.select_mode_flush();
            }
        }
        (mirrored, failed)
    }

    pub fn select_by_material(&mut self, material: i32, select: bool) -> bool {
        let mut changed = false;
        for face in self.faces().collect::<Vec<_>>() {
            if !self.is_face_hidden(face) && self.face_material(face) == material {
                self.select_face(face, select);
                changed = true;
            }
        }
        if changed {
            self.select_mode_flush();
        }
        changed
    }

    pub fn select_verts_where(
        &mut self,
        select: bool,
        test: impl Fn(&EditMesh, VertId) -> bool,
    ) -> bool {
        let chosen: Vec<VertId> = self
            .verts()
            .filter(|&vert| !self.is_vert_hidden(vert) && test(self, vert))
            .collect();
        for &vert in &chosen {
            self.select_vert(vert, select);
        }
        match select {
            true => self.select_flush_from_verts(true),
            false => self.select_flush_from_verts(false),
        }
        !chosen.is_empty()
    }

    pub fn select_vertex_group(&mut self, group: u32, select: bool) -> bool {
        self.select_verts_where(select, |mesh, vert| {
            mesh.vert_influences(vert)
                .iter()
                .any(|influence| influence.group == group)
        })
    }

    pub fn select_ungrouped(&mut self, extend: bool) -> bool {
        if self.layout.vertex_groups.is_empty() {
            return false;
        }
        if !extend {
            self.deselect_all();
        }
        let changed =
            self.select_verts_where(true, |mesh, vert| mesh.vert_influences(vert).is_empty());
        self.select_mode_flush();
        changed
    }

    pub fn select_previous_active(&mut self) -> bool {
        let Some(&last) = self.history.last() else {
            return false;
        };
        self.select_element(last, false);
        if self.history.len() >= 2
            && let Element::Face(face) = self.history[self.history.len() - 2]
        {
            self.active_face = Some(face.0);
        }
        self.select_mode_flush();
        true
    }

    pub fn is_edge_uv_contiguous(&self, edge: EdgeId, uv_map: &str) -> bool {
        let Some(uvs) = self.loop_uvs(uv_map) else {
            return true;
        };
        let Some(first) = self.edge_first_loop(edge) else {
            return true;
        };
        let base_a = first;
        let base_b = self.loop_next(first);
        let mut lp: LoopId = self.loop_radial_next(first);
        while lp != first {
            let (a, b) = match self.loop_vert(lp) == self.loop_vert(base_a) {
                true => (lp, self.loop_next(lp)),
                false => (self.loop_next(lp), lp),
            };
            if uvs[a.index()] != uvs[base_a.index()] || uvs[b.index()] != uvs[base_b.index()] {
                return false;
            }
            lp = self.loop_radial_next(lp);
        }
        true
    }
}

#[derive(Default)]
struct Geometry3 {
    verts: IdSet<VertId>,
    edges: IdSet<EdgeId>,
    faces: IdSet<FaceId>,
    visited_verts: IdSet<VertId>,
}

enum LinkedSeeds {
    Verts(Vec<VertId>),
    Faces(Vec<FaceId>),
}
