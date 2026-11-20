use glam::Vec3;

use crate::heap::SimpleHeap;
use crate::ids::{EdgeId, Element, ElementId, FaceId, VertId};
use crate::math::{line_point_factor, normalize_with_length};
use crate::mesh::EditMesh;
use crate::select_ops::Checker;
use crate::sets::IdSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EdgePathTag {
    #[default]
    Select,
    Seam,
    Sharp,
    Crease,
    BevelWeight,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PathOptions {
    pub topology_distance: bool,
    pub face_step: bool,
    pub fill: bool,
    pub edge_tag: EdgePathTag,
    pub interval: Option<Checker>,
}

struct Search<I: ElementId> {
    heap: SimpleHeap<I>,
    cost: Vec<f32>,
    previous: Vec<Option<I>>,
}

impl<I: ElementId> Search<I> {
    fn new(capacity: usize, start: I) -> Search<I> {
        let mut search = Search {
            heap: SimpleHeap::new(),
            cost: vec![f32::MAX; capacity],
            previous: vec![None; capacity],
        };
        search.cost[start.index()] = 0.0;
        search.push(start, 0.0);
        search
    }

    fn push(&mut self, element: I, cost: f32) {
        self.heap.insert(cost, element);
    }

    fn relax(&mut self, from: I, to: I, step: f32) {
        let cost = self.cost[from.index()] + step;
        if self.cost[to.index()] > cost {
            self.cost[to.index()] = cost;
            self.previous[to.index()] = Some(from);
            self.push(to, cost);
        }
    }

    fn path(&self, end: I) -> Vec<I> {
        let mut path = vec![end];
        let mut current = end;
        while let Some(previous) = self.previous[current.index()] {
            path.push(previous);
            current = previous;
        }
        path.reverse();
        path
    }
}

fn step_cost(a: Vec3, b: Vec3, c: Vec3, skip_ab: bool, skip_bc: bool) -> f32 {
    let (along_ab, length_ab) = normalize_with_length(b - a);
    let (along_bc, length_bc) = normalize_with_length(c - b);
    let cost = (if skip_ab { 0.0 } else { length_ab }) + (if skip_bc { 0.0 } else { length_bc });
    cost * (1.0 + 0.5 * (2.0 - along_ab.dot(along_bc).abs().sqrt()))
}

impl EditMesh {
    #[must_use]
    pub fn shortest_path_verts(
        &self,
        from: VertId,
        to: VertId,
        options: &PathOptions,
    ) -> Vec<VertId> {
        let mut blocked: IdSet<VertId> = self
            .verts()
            .filter(|&vert| self.is_vert_hidden(vert))
            .collect();
        let mut search = Search::new(self.vert_capacity(), from);
        let mut reached = false;
        while let Some(vert) = search.heap.pop_min() {
            if vert == to {
                reached = true;
                break;
            }
            if !blocked.insert(vert) {
                continue;
            }
            for edge in self.vert_edges(vert) {
                let Some(other) = self.edge_other_vert(edge, vert) else {
                    continue;
                };
                if !blocked.contains(other) {
                    let step = match options.topology_distance {
                        true => 1.0,
                        false => self.position(vert).distance(self.position(other)),
                    };
                    search.relax(vert, other, step);
                }
            }
            if options.face_step {
                for lp in self.vert_loops(vert) {
                    if self.face_len(self.loop_face(lp)) <= 3 {
                        continue;
                    }
                    let end = self.loop_prev(lp);
                    let mut other = self.loop_next(self.loop_next(lp));
                    while other != end {
                        let target = self.loop_vert(other);
                        if !blocked.contains(target) {
                            let step = match options.topology_distance {
                                true => 1.0,
                                false => self.position(vert).distance(self.position(target)),
                            };
                            search.relax(vert, target, step);
                        }
                        other = self.loop_next(other);
                    }
                }
            }
        }
        match reached {
            true => search.path(to),
            false => Vec::new(),
        }
    }

    #[must_use]
    pub fn shortest_path_edges(
        &self,
        from: EdgeId,
        to: EdgeId,
        options: &PathOptions,
    ) -> Vec<EdgeId> {
        let mut blocked_verts: IdSet<VertId> = IdSet::new();
        let mut blocked: IdSet<EdgeId> = self
            .edges()
            .filter(|&edge| self.is_edge_hidden(edge))
            .collect();
        let mut search = Search::new(self.edge_capacity(), from);
        let mut reached = false;
        while let Some(edge) = search.heap.pop_min() {
            if edge == to {
                reached = true;
                break;
            }
            if !blocked.insert(edge) {
                continue;
            }
            blocked_verts.extend(self.edge_verts(edge));
            if !options.face_step || self.is_edge_wire(edge) {
                for vert in self.edge_verts(edge) {
                    if search.previous[edge.index()]
                        .is_some_and(|previous| self.edge_has_vert(previous, vert))
                    {
                        continue;
                    }
                    for other in self.vert_edges(vert) {
                        let far = self.edge_other_vert(other, vert);
                        if blocked.contains(other)
                            || far.is_some_and(|far| blocked_verts.contains(far))
                        {
                            continue;
                        }
                        let step = match options.topology_distance {
                            true => 1.0,
                            false => {
                                let a = self.edge_other_vert(edge, vert).unwrap_or(vert);
                                let b = far.unwrap_or(vert);
                                step_cost(
                                    self.position(a),
                                    self.position(vert),
                                    self.position(b),
                                    false,
                                    false,
                                )
                            }
                        };
                        search.relax(edge, other, step);
                    }
                }
            } else {
                for lp in self.edge_loops(edge).collect::<Vec<_>>() {
                    let face = self.loop_face(lp);
                    let (mut current, end) = match self.face_len(face) > 3 {
                        true => (self.loop_next(self.loop_next(lp)), self.loop_prev(lp)),
                        false => (self.loop_next(lp), lp),
                    };
                    while current != end {
                        let other = self.loop_edge(current);
                        if !blocked.contains(other) {
                            let step = match options.topology_distance {
                                true => 1.0,
                                false => {
                                    let a = self.position(self.edge_verts(edge)[0]);
                                    let b = self.position(self.edge_verts(other)[0]);
                                    step_cost(
                                        a,
                                        b,
                                        self.face_center_median_weighted(face),
                                        false,
                                        false,
                                    )
                                }
                            };
                            search.relax(edge, other, step);
                        }
                        current = self.loop_next(current);
                    }
                }
            }
        }
        match reached {
            true => search.path(to),
            false => Vec::new(),
        }
    }

    fn face_cut_cost_edge(&self, a: FaceId, b: FaceId, edge: EdgeId, ends: [FaceId; 2]) -> f32 {
        let center_a = self.face_center_median_weighted(a);
        let center_b = self.face_center_median_weighted(b);
        let [v1, v2] = self.edge_verts(edge).map(|vert| self.position(vert));
        let crossing = closest_on_lines(v1, v2, center_a, center_b).unwrap_or((v1 + v2) * 0.5);
        let factor = line_point_factor(crossing, v1, v2);
        let middle = if factor < 0.0 {
            v1
        } else if factor > 1.0 {
            v2
        } else {
            crossing
        };
        step_cost(center_a, middle, center_b, a == ends[0], b == ends[1])
    }

    #[must_use]
    pub fn shortest_path_faces(
        &self,
        from: FaceId,
        to: FaceId,
        options: &PathOptions,
    ) -> Vec<FaceId> {
        let ends = [from, to];
        let mut blocked: IdSet<FaceId> = self
            .faces()
            .filter(|&face| self.is_face_hidden(face))
            .collect();
        let mut search = Search::new(self.face_capacity(), from);
        let mut reached = false;
        while let Some(face) = search.heap.pop_min() {
            if face == to {
                reached = true;
                break;
            }
            if !blocked.insert(face) {
                continue;
            }
            for lp in self.face_loops(face) {
                for radial in self.edge_loops(self.loop_edge(lp)) {
                    let other = self.loop_face(radial);
                    if blocked.contains(other) {
                        continue;
                    }
                    let step = match options.topology_distance {
                        true => 1.0,
                        false => self.face_cut_cost_edge(face, other, self.loop_edge(lp), ends),
                    };
                    search.relax(face, other, step);
                }
            }
            if options.face_step {
                for lp in self.face_loops(face) {
                    let vert = self.loop_vert(lp);
                    for around in self.vert_loops(vert) {
                        let shares_edge = self.loop_edge(around) == self.loop_edge(lp)
                            || self.loop_edge(around) == self.loop_edge(self.loop_prev(lp))
                            || self.loop_edge(self.loop_prev(around)) == self.loop_edge(lp)
                            || self.loop_edge(self.loop_prev(around))
                                == self.loop_edge(self.loop_prev(lp));
                        if around == lp || shares_edge {
                            continue;
                        }
                        let other = self.loop_face(around);
                        if blocked.contains(other) {
                            continue;
                        }
                        let step = match options.topology_distance {
                            true => 1.0,
                            false => step_cost(
                                self.face_center_median_weighted(face),
                                self.position(vert),
                                self.face_center_median_weighted(other),
                                face == ends[0],
                                other == ends[1],
                            ),
                        };
                        search.relax(face, other, step);
                    }
                }
            }
        }
        match reached {
            true => search.path(to),
            false => Vec::new(),
        }
    }

    fn is_vert_edge_pair_manifold(&self, vert: VertId) -> bool {
        match self.vert_edge_pair(vert) {
            Some((a, b)) => self.is_edge_manifold(a) && self.is_edge_manifold(b),
            None => false,
        }
    }

    fn vert_pair_ends(&self, pivot: VertId) -> Option<[VertId; 2]> {
        let mut ends = [pivot; 2];
        for (slot, edge) in self.vert_edges(pivot).enumerate().take(2) {
            let mut chain = edge;
            let mut other = self.edge_other_vert(edge, pivot)?;
            while self.is_vert_edge_pair_manifold(other) {
                let next = self.disk_next(chain, other);
                other = self.edge_other_vert(next, other)?;
                if other == pivot {
                    return None;
                }
                chain = next;
            }
            ends[slot] = other;
        }
        Some(ends)
    }

    fn region_vert_test(&self, vert: VertId, depths: &[Vec<i32>; 2], pass: i32) -> bool {
        let index = vert.index();
        depths[0][index] != -1
            && depths[1][index] != -1
            && depths[0][index] + depths[1][index] < pass
    }

    fn region_vert_test_chain(&self, vert: VertId, depths: &[Vec<i32>; 2], pass: i32) -> bool {
        if self.region_vert_test(vert, depths, pass) {
            return true;
        }
        self.is_vert_edge_pair_manifold(vert)
            && self.vert_pair_ends(vert).is_some_and(|[a, b]| {
                self.region_vert_test(a, depths, pass) && self.region_vert_test(b, depths, pass)
            })
    }

    fn element_verts(&self, element: Element) -> Vec<VertId> {
        match element {
            Element::Vert(vert) => vec![vert],
            Element::Edge(edge) => self.edge_verts(edge).to_vec(),
            Element::Face(face) => self.face_verts_vec(face),
        }
    }

    #[must_use]
    pub fn path_region(&self, from: Element, to: Element) -> Vec<Element> {
        let kind = to.kind();
        let mut blocked: IdSet<VertId> = IdSet::new();
        match kind {
            crate::ElementKind::Vert => {
                blocked.extend(self.verts().filter(|&vert| self.is_vert_hidden(vert)));
            }
            crate::ElementKind::Edge => {
                blocked.extend(self.verts());
                for edge in self.edges().filter(|&edge| !self.is_edge_hidden(edge)) {
                    for vert in self.edge_verts(edge) {
                        blocked.remove(vert);
                    }
                }
            }
            crate::ElementKind::Face => {
                blocked.extend(self.verts());
                for face in self.faces().filter(|&face| !self.is_face_hidden(face)) {
                    for vert in self.face_verts(face) {
                        blocked.remove(vert);
                    }
                }
            }
        }
        let sides = [self.element_verts(from), self.element_verts(to)];
        let mut depths = [
            vec![-1i32; self.vert_capacity()],
            vec![-1i32; self.vert_capacity()],
        ];
        let mut pass = 0;
        let mut found_all = false;
        for side in 0..2 {
            let other_side = 1 - side;
            let mut stack: Vec<VertId> = Vec::new();
            let mut stack_other: Vec<VertId> = Vec::new();
            for &vert in &sides[side] {
                depths[side][vert.index()] = 0;
                if !self.is_vert_loose(vert) && !blocked.contains(vert) {
                    stack.push(vert);
                }
            }
            if kind != crate::ElementKind::Face {
                for &vert in &sides[side] {
                    if self.is_vert_edge_pair_manifold(vert)
                        && let Some(ends) = self.vert_pair_ends(vert)
                    {
                        for end in ends {
                            if depths[side][end.index()] == -1 {
                                depths[side][end.index()] = 0;
                                if !blocked.contains(end) {
                                    stack.push(end);
                                }
                            }
                        }
                    }
                }
            }
            found_all = false;
            pass = 1;
            while !stack.is_empty() {
                while let Some(vert) = stack.pop() {
                    for edge in self.vert_edges(vert).collect::<Vec<_>>() {
                        let Some(mut other) = self.edge_other_vert(edge, vert) else {
                            continue;
                        };
                        if depths[side][other.index()] != -1 {
                            continue;
                        }
                        let mut chain = edge;
                        while self.is_vert_edge_pair_manifold(other)
                            && depths[side][other.index()] == -1
                        {
                            depths[side][other.index()] = pass;
                            let next = self.disk_next(chain, other);
                            let Some(far) = self.edge_other_vert(next, other) else {
                                break;
                            };
                            other = far;
                            chain = next;
                        }
                        if depths[side][other.index()] == -1 {
                            depths[side][other.index()] = pass;
                            if !blocked.contains(other) {
                                stack_other.push(other);
                            }
                        }
                    }
                }
                found_all = sides[other_side]
                    .iter()
                    .all(|vert| depths[side][vert.index()] != -1);
                if found_all {
                    pass += 1;
                    break;
                }
                std::mem::swap(&mut stack, &mut stack_other);
                pass += 1;
            }
            if !found_all {
                break;
            }
        }
        if !found_all {
            return Vec::new();
        }
        let mut out = Vec::new();
        match kind {
            crate::ElementKind::Face => {
                for face in self.faces() {
                    if self.is_face_hidden(face) {
                        continue;
                    }
                    let mut allowance = usize::from(self.face_len(face) > 3);
                    let ok = self.face_verts(face).all(|vert| {
                        if self.region_vert_test_chain(vert, &depths, pass) {
                            return true;
                        }
                        match allowance {
                            0 => false,
                            _ => {
                                allowance -= 1;
                                true
                            }
                        }
                    });
                    if ok {
                        out.push(Element::Face(face));
                    }
                }
            }
            crate::ElementKind::Edge => {
                for edge in self.edges() {
                    if !self.is_edge_hidden(edge)
                        && self
                            .edge_verts(edge)
                            .iter()
                            .all(|&vert| self.region_vert_test_chain(vert, &depths, pass))
                    {
                        out.push(Element::Edge(edge));
                    }
                }
            }
            crate::ElementKind::Vert => {
                for vert in self.verts() {
                    if self.region_vert_test_chain(vert, &depths, pass) {
                        out.push(Element::Vert(vert));
                    }
                }
            }
        }
        out.reverse();
        out
    }

    #[must_use]
    pub fn shortest_path(&self, from: Element, to: Element, options: &PathOptions) -> Vec<Element> {
        if options.fill {
            return self.path_region(from, to);
        }
        match (from, to) {
            (Element::Vert(a), Element::Vert(b)) => self
                .shortest_path_verts(a, b, options)
                .into_iter()
                .map(Element::Vert)
                .collect(),
            (Element::Edge(a), Element::Edge(b)) => self
                .shortest_path_edges(a, b, options)
                .into_iter()
                .map(Element::Edge)
                .collect(),
            (Element::Face(a), Element::Face(b)) => self
                .shortest_path_faces(a, b, options)
                .into_iter()
                .map(Element::Face)
                .collect(),
            _ => Vec::new(),
        }
    }

    fn path_element_set(&self, element: Element, options: &PathOptions) -> bool {
        match (element, options.edge_tag) {
            (Element::Edge(edge), EdgePathTag::Seam) => self.is_edge_seam(edge),
            (Element::Edge(edge), EdgePathTag::Sharp) => self.is_edge_sharp(edge),
            (Element::Edge(edge), EdgePathTag::Crease) => self
                .edge_creases()
                .is_some_and(|values| values[edge.index()] != 0.0),
            (Element::Edge(edge), EdgePathTag::BevelWeight) => self
                .edge_bevel_weights()
                .is_some_and(|values| values[edge.index()] != 0.0),
            (element, _) => self.is_element_selected(element),
        }
    }

    fn path_element_apply(&mut self, element: Element, value: bool, options: &PathOptions) {
        match (element, options.edge_tag) {
            (Element::Edge(edge), EdgePathTag::Seam) => self.set_edge_seam(edge, value),
            (Element::Edge(edge), EdgePathTag::Sharp) => self.set_edge_sharp(edge, value),
            (Element::Edge(edge), EdgePathTag::Crease) => {
                self.edge_creases_mut()[edge.index()] = if value { 1.0 } else { 0.0 };
            }
            (Element::Edge(edge), EdgePathTag::BevelWeight) => {
                self.edge_bevel_weights_mut()[edge.index()] = if value { 1.0 } else { 0.0 };
            }
            (element, _) => self.select_element(element, value),
        }
    }

    pub fn select_shortest_path(&mut self, to: Element, options: &PathOptions) -> bool {
        let Some(from) = self.active_element() else {
            self.path_element_apply(to, true, options);
            self.set_active(to);
            self.select_mode_flush();
            return true;
        };
        if from.kind() != to.kind() || from == to {
            return false;
        }
        let path = self.shortest_path(from, to, options);
        if path.is_empty() {
            return false;
        }
        let ordered = !options.fill;
        let all_set = path
            .iter()
            .all(|&element| self.path_element_set(element, options));
        let mut last = to;
        for (index, &element) in path.iter().enumerate() {
            let depth = index as i64 - 1;
            let include = !ordered
                || options
                    .interval
                    .is_none_or(|interval| interval.keeps(depth));
            if include {
                self.path_element_apply(element, !all_set, options);
                if ordered {
                    last = element;
                }
            }
        }
        if options.edge_tag == EdgePathTag::Select || !matches!(to, Element::Edge(_)) {
            self.select_mode_flush();
            if self.is_element_selected(last) {
                self.set_active(last);
            }
        }
        true
    }
}

fn closest_on_lines(a1: Vec3, a2: Vec3, b1: Vec3, b2: Vec3) -> Option<Vec3> {
    let along_a = a2 - a1;
    let along_b = b2 - b1;
    let normal = along_a.cross(along_b);
    let squared = normal.length_squared();
    if squared == 0.0 {
        return None;
    }
    let offset = b1 - a1;
    let factor = offset.cross(along_b).dot(normal) / squared;
    Some(a1 + along_a * factor)
}
