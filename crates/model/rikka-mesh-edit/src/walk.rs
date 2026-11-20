use std::collections::VecDeque;

use bitflags::bitflags;
use glam::Vec2;

use crate::ids::{EdgeId, FaceId, LoopId, VertId};
use crate::mesh::EditMesh;
use crate::sets::IdSet;

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct WalkDelimit: u8 {
        const SEAM = 1;
        const SHARP = 1 << 1;
        const MATERIAL = 1 << 2;
        const LOOP_INNER_CORNERS = 1 << 3;
        const LOOP_OUTER_CORNERS = 1 << 4;
        const LOOP_NGONS = 1 << 5;
        const RING_NGONS = 1 << 6;
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Walk<'a> {
    pub skip_hidden: bool,
    pub delimit: WalkDelimit,
    pub verts: Option<&'a IdSet<VertId>>,
    pub edges: Option<&'a IdSet<EdgeId>>,
    pub faces: Option<&'a IdSet<FaceId>>,
}

impl<'a> Walk<'a> {
    #[must_use]
    pub fn visible() -> Walk<'a> {
        Walk {
            skip_hidden: true,
            ..Walk::default()
        }
    }

    #[must_use]
    pub fn delimit(mut self, delimit: WalkDelimit) -> Walk<'a> {
        self.delimit = delimit;
        self
    }

    fn vert_ok(&self, mesh: &EditMesh, vert: VertId) -> bool {
        !(self.skip_hidden && mesh.is_vert_hidden(vert))
            && self.verts.is_none_or(|mask| mask.contains(vert))
    }

    fn edge_ok(&self, mesh: &EditMesh, edge: EdgeId) -> bool {
        !(self.skip_hidden && mesh.is_edge_hidden(edge))
            && self.edges.is_none_or(|mask| mask.contains(edge))
    }

    fn face_ok(&self, mesh: &EditMesh, face: FaceId) -> bool {
        !(self.skip_hidden && mesh.is_face_hidden(face))
            && self.faces.is_none_or(|mask| mask.contains(face))
    }
}

struct States<S> {
    queue: VecDeque<S>,
    breadth: bool,
}

impl<S: Copy> States<S> {
    fn new(breadth: bool) -> States<S> {
        States {
            queue: VecDeque::new(),
            breadth,
        }
    }

    fn add(&mut self, state: S) {
        match self.breadth {
            true => self.queue.push_back(state),
            false => self.queue.push_front(state),
        }
    }

    fn current(&self) -> Option<S> {
        self.queue.front().copied()
    }

    fn remove(&mut self) -> Option<S> {
        self.queue.pop_front()
    }
}

#[derive(Clone, Copy)]
struct LoopState {
    current: EdgeId,
    last: VertId,
    boundary: bool,
    single: bool,
    hub: Option<FaceId>,
}

#[derive(Clone, Copy)]
struct FaceLoopState {
    lp: LoopId,
    no_calc: bool,
}

#[derive(Clone, Copy)]
struct RingState {
    lp: Option<LoopId>,
    wire: EdgeId,
    no_calc: bool,
}

#[derive(Clone, Copy)]
struct NonManifoldState {
    start: EdgeId,
    start_vert: VertId,
    current: EdgeId,
    last: VertId,
    face_count: usize,
}

impl EditMesh {
    fn vert_edge_count_nonwire(&self, vert: VertId) -> usize {
        self.vert_edges(vert)
            .filter(|&edge| !self.is_edge_wire(edge))
            .count()
    }

    fn is_edge_single(&self, edge: EdgeId) -> bool {
        if !self.is_edge_boundary(edge) {
            return false;
        }
        let Some(lp) = self.edge_first_loop(edge) else {
            return false;
        };
        self.face_len(self.loop_face(lp)) > 4
            && (self.is_edge_boundary(self.loop_edge(self.loop_next(lp)))
                || self.is_edge_boundary(self.loop_edge(self.loop_prev(lp))))
    }

    fn loop_delimit_by_mark(
        &self,
        walk: &Walk,
        vert: VertId,
        edge: EdgeId,
        lp: LoopId,
        marked: impl Fn(EdgeId) -> bool,
    ) -> bool {
        let usable = |other: EdgeId| {
            !(walk.skip_hidden && self.is_edge_hidden(other)) && !self.is_edge_wire(other)
        };
        if marked(edge) {
            let next = self.loop_edge(lp);
            !marked(next) && usable(next)
        } else {
            self.vert_edges(vert)
                .any(|other| marked(other) && usable(other))
        }
    }

    fn loop_delimit_mark_check(&self, walk: &Walk, vert: VertId, edge: EdgeId, lp: LoopId) -> bool {
        (walk.delimit.contains(WalkDelimit::SEAM)
            && self.loop_delimit_by_mark(walk, vert, edge, lp, |other| self.is_edge_seam(other)))
            || (walk.delimit.contains(WalkDelimit::SHARP)
                && self
                    .loop_delimit_by_mark(walk, vert, edge, lp, |other| self.is_edge_sharp(other)))
    }

    #[must_use]
    pub fn walk_edge_loop(&self, start: EdgeId, walk: &Walk) -> Vec<EdgeId> {
        let [v1, v2] = self.edge_verts(start);
        let edge_counts = [
            self.vert_edge_count_nonwire(v1),
            self.vert_edge_count_nonwire(v2),
        ];
        let face_counts = [self.vert_face_count(v1), self.vert_face_count(v2)];
        let boundary = self.is_edge_boundary(start);
        let single = boundary && self.is_edge_single(start);
        let hub = match !boundary
            && ((edge_counts[0] == 3 && face_counts[0] == 3)
                || (edge_counts[1] == 3 && face_counts[1] == 3))
        {
            true => self
                .edge_faces(start)
                .fold(None::<FaceId>, |best, face| match best {
                    Some(best) if self.face_len(best) >= self.face_len(face) => Some(best),
                    _ => Some(face),
                })
                .filter(|&face| self.face_len(face) > 4),
            false => None,
        };
        let mut visited: IdSet<EdgeId> = IdSet::new();
        let mut states = States::new(false);
        visited.insert(start);
        states.add(LoopState {
            current: start,
            last: v1,
            boundary,
            single,
            hub,
        });
        let mut far = states.current().expect("state was added");
        while let Some(state) = states.current() {
            far = state;
            self.edge_loop_walk(&mut states, &mut visited, walk);
        }
        let mut restart = far;
        restart.last = self
            .edge_other_vert(far.current, far.last)
            .unwrap_or(far.last);
        visited.clear();
        visited.insert(far.current);
        states.add(restart);
        let mut out = Vec::new();
        while let Some(edge) = self.edge_loop_walk(&mut states, &mut visited, walk) {
            out.push(edge);
        }
        out
    }

    fn edge_loop_walk(
        &self,
        states: &mut States<LoopState>,
        visited: &mut IdSet<EdgeId>,
        walk: &Walk,
    ) -> Option<EdgeId> {
        let state = states.remove()?;
        self.edge_loop_step(state, states, visited, walk);
        Some(state.current)
    }

    fn edge_loop_step(
        &self,
        state: LoopState,
        states: &mut States<LoopState>,
        visited: &mut IdSet<EdgeId>,
        walk: &Walk,
    ) {
        let edge = state.current;
        let push = |next: EdgeId,
                    vert: VertId,
                    states: &mut States<LoopState>,
                    visited: &mut IdSet<EdgeId>| {
            states.add(LoopState {
                current: next,
                last: vert,
                ..state
            });
            visited.insert(next);
        };
        let first_loop = self.edge_first_loop(edge);
        if let Some(hub) = state.hub {
            let Some(vert) = self.edge_other_vert(edge, state.last) else {
                return;
            };
            if self.vert_edge_count_nonwire(vert) == 3
                && let Some(lp) = self.face_other_vert_loop(hub, state.last, vert)
                && let Some(next) = self.edge_between(vert, self.loop_vert(lp))
                && walk.edge_ok(self, next)
                && !visited.contains(next)
                && !self.is_edge_boundary(next)
            {
                push(next, vert, states, visited);
            }
            return;
        }
        let Some(mut lp) = first_loop else {
            for vert in self.edge_verts(edge) {
                let candidates: Vec<EdgeId> = self.vert_edges(vert).collect();
                for next in candidates {
                    if self.is_edge_wire(next)
                        && walk.edge_ok(self, next)
                        && !visited.contains(next)
                    {
                        push(next, vert, states, visited);
                    }
                }
            }
            return;
        };
        let Some(vert) = self.edge_other_vert(edge, state.last) else {
            return;
        };
        let total = self.vert_edge_count_nonwire(vert);
        let mut found = true;
        if !state.boundary {
            if total == 4 || total == 2 {
                let opposite = total / 2;
                for _ in 0..opposite {
                    lp = self.loop_other_edge_loop(lp, vert);
                    if self.is_edge_manifold(self.loop_edge(lp)) {
                        lp = self.loop_radial_next(lp);
                    } else {
                        found = false;
                        break;
                    }
                }
            } else {
                found = false;
            }
            if found && self.loop_delimit_mark_check(walk, vert, edge, lp) {
                found = false;
            }
        } else {
            let mut corner = false;
            if walk.delimit.contains(WalkDelimit::LOOP_INNER_CORNERS) && total > 3 {
                corner = true;
            }
            if walk.delimit.contains(WalkDelimit::LOOP_OUTER_CORNERS)
                && !corner
                && total == 2
                && !self.is_edge_single(edge)
            {
                corner = true;
            }
            if !corner {
                loop {
                    lp = self.loop_other_edge_loop(lp, vert);
                    let candidate = self.loop_edge(lp);
                    if self.is_edge_manifold(candidate) {
                        lp = self.loop_radial_next(lp);
                    } else if self.is_edge_boundary(candidate) {
                        break;
                    } else {
                        found = false;
                        break;
                    }
                }
            }
            if found && self.loop_delimit_mark_check(walk, vert, edge, lp) {
                found = false;
            }
            if found
                && walk.delimit.contains(WalkDelimit::LOOP_NGONS)
                && state.single != self.is_edge_single(self.loop_edge(lp))
            {
                found = false;
            }
        }
        if found {
            let next = self.loop_edge(lp);
            if Some(lp) != first_loop && walk.edge_ok(self, next) && !visited.contains(next) {
                push(next, vert, states, visited);
            }
        }
    }

    fn face_loop_includes(
        &self,
        walk: &Walk,
        lp: LoopId,
        faces: &IdSet<FaceId>,
        edges: &IdSet<EdgeId>,
    ) -> bool {
        let face = self.loop_face(lp);
        self.face_len(face) == 4
            && walk.face_ok(self, face)
            && !(faces.contains(face) && edges.contains(self.loop_edge(lp)))
    }

    fn face_loop_delimited(
        &self,
        a: FaceId,
        edge: EdgeId,
        b: FaceId,
        delimit: WalkDelimit,
    ) -> bool {
        (delimit.contains(WalkDelimit::SEAM) && self.is_edge_seam(edge))
            || (delimit.contains(WalkDelimit::SHARP) && self.is_edge_sharp(edge))
            || (delimit.contains(WalkDelimit::MATERIAL)
                && self.face_material(a) != self.face_material(b))
    }

    #[must_use]
    pub fn walk_face_loop(&self, start: EdgeId, walk: &Walk) -> Vec<FaceId> {
        let mut faces: IdSet<FaceId> = IdSet::new();
        let mut edges: IdSet<EdgeId> = IdSet::new();
        let begins = !self.is_edge_wire(start)
            && self.is_edge_manifold(start)
            && !(self.is_edge_boundary(start)
                && !self.face_loop_includes(
                    walk,
                    self.edge_first_loop(start)
                        .expect("boundary edge has a loop"),
                    &faces,
                    &edges,
                ));
        if !begins {
            return Vec::new();
        }
        let first = self
            .edge_first_loop(start)
            .expect("manifold edge has loops");
        let mut states = States::new(false);
        states.add(FaceLoopState {
            lp: first,
            no_calc: false,
        });
        faces.insert(self.loop_face(first));
        let mut far = states.current().expect("state was added");
        while let Some(state) = states.current() {
            far = state;
            self.face_loop_walk(&mut states, &mut faces, &mut edges, walk);
        }
        let restart = FaceLoopState {
            lp: far.lp,
            no_calc: false,
        };
        edges.clear();
        edges.insert(self.loop_edge(restart.lp));
        faces.clear();
        faces.insert(self.loop_face(restart.lp));
        states.add(restart);
        let radial = self.loop_radial_next(restart.lp);
        if self.face_loop_delimited(
            self.loop_face(restart.lp),
            self.loop_edge(restart.lp),
            self.loop_face(radial),
            walk.delimit,
        ) {
            states.add(FaceLoopState {
                lp: radial,
                no_calc: false,
            });
            faces.insert(self.loop_face(radial));
        }
        let mut out = Vec::new();
        while let Some(face) = self.face_loop_walk(&mut states, &mut faces, &mut edges, walk) {
            out.push(face);
        }
        out
    }

    fn face_loop_walk(
        &self,
        states: &mut States<FaceLoopState>,
        faces: &mut IdSet<FaceId>,
        edges: &mut IdSet<EdgeId>,
        walk: &Walk,
    ) -> Option<FaceId> {
        let state = states.remove()?;
        let face = self.loop_face(state.lp);
        if state.no_calc {
            return Some(face);
        }
        let mut lp = self.loop_radial_next(state.lp);
        if !self.face_loop_includes(walk, lp, faces, edges) {
            lp = self.loop_next(self.loop_next(state.lp));
            if !self.is_edge_manifold(self.loop_edge(lp)) {
                lp = self.loop_prev(self.loop_prev(lp));
            }
            lp = self.loop_radial_next(lp);
        }
        if self.face_loop_includes(walk, lp, faces, edges) {
            let next_face = self.loop_face(lp);
            let stop = self.face_len(next_face) != 4
                || self.face_loop_delimited(face, self.loop_edge(lp), next_face, walk.delimit);
            states.add(FaceLoopState {
                lp: if stop { state.lp } else { lp },
                no_calc: stop,
            });
            edges.insert(self.loop_edge(lp));
            faces.insert(next_face);
        }
        Some(face)
    }

    fn ring_delimited(&self, edge: EdgeId, delimit: WalkDelimit) -> bool {
        if !self.is_edge_manifold(edge) {
            return false;
        }
        if delimit.contains(WalkDelimit::SEAM) && self.is_edge_seam(edge) {
            return true;
        }
        if delimit.contains(WalkDelimit::SHARP) && self.is_edge_sharp(edge) {
            return true;
        }
        if delimit.contains(WalkDelimit::MATERIAL)
            && let Some((a, b)) = self.edge_face_pair(edge)
            && self.face_material(a) != self.face_material(b)
        {
            return true;
        }
        false
    }

    #[must_use]
    pub fn walk_edge_ring(&self, start: EdgeId, walk: &Walk) -> Vec<EdgeId> {
        let mut states = States::new(false);
        let mut visited: IdSet<EdgeId> = IdSet::new();
        let first = self.edge_first_loop(start);
        if first.is_none() || self.ring_delimited(start, walk.delimit) {
            states.add(RingState {
                lp: None,
                wire: start,
                no_calc: false,
            });
        } else {
            visited.insert(start);
            states.add(RingState {
                lp: first,
                wire: start,
                no_calc: false,
            });
            let mut far = states.current().expect("state was added");
            while let Some(state) = states.current() {
                far = state;
                self.edge_ring_walk(&mut states, &mut visited, walk);
            }
            let delimit_ngon = walk.delimit.contains(WalkDelimit::RING_NGONS);
            let mut restart = far;
            let odd = |mesh: &EditMesh, lp: LoopId| {
                let len = mesh.face_len(mesh.loop_face(lp));
                match delimit_ngon {
                    true => len != 4,
                    false => !len.is_multiple_of(2),
                }
            };
            let mut lp = restart.lp.expect("ring state holds a loop");
            if odd(self, lp) {
                lp = self.loop_radial_next(lp);
                restart.lp = Some(lp);
            }
            visited.clear();
            visited.insert(self.loop_edge(lp));
            states.add(restart);
            if self.loop_radial_next(lp) != lp && (restart.no_calc || odd(self, lp)) {
                states.add(RingState {
                    lp: Some(self.loop_radial_next(lp)),
                    wire: start,
                    no_calc: false,
                });
            }
        }
        let mut out = Vec::new();
        while let Some(edge) = self.edge_ring_walk(&mut states, &mut visited, walk) {
            out.push(edge);
        }
        out
    }

    fn edge_ring_walk(
        &self,
        states: &mut States<RingState>,
        visited: &mut IdSet<EdgeId>,
        walk: &Walk,
    ) -> Option<EdgeId> {
        let state = states.remove()?;
        let Some(start) = state.lp else {
            return Some(state.wire);
        };
        let edge = self.loop_edge(start);
        let check = |candidate: EdgeId| {
            walk.edge_ok(self, candidate)
                && (self.is_edge_boundary(candidate) || self.is_edge_manifold(candidate))
        };
        if !check(edge) || state.no_calc {
            return Some(edge);
        }
        let delimit_ngon = walk.delimit.contains(WalkDelimit::RING_NGONS);
        let (lp, ok) = if delimit_ngon {
            let mut lp = self.loop_next(self.loop_next(self.loop_radial_next(start)));
            if self.face_len(self.loop_face(lp)) != 4
                || !check(self.loop_edge(lp))
                || !walk.face_ok(self, self.loop_face(lp))
            {
                lp = self.loop_next(self.loop_next(start));
            }
            let ok = self.face_len(self.loop_face(lp)) == 4
                && check(self.loop_edge(lp))
                && !visited.contains(self.loop_edge(lp));
            (lp, ok)
        } else {
            let opposite = |from: LoopId| {
                let len = self.face_len(self.loop_face(from));
                let mut lp = from;
                let mut remaining = len as isize;
                while remaining > 0 {
                    lp = self.loop_next(lp);
                    remaining -= 2;
                }
                (lp, len)
            };
            let (mut lp, len) = opposite(self.loop_radial_next(start));
            if !len.is_multiple_of(2)
                || !check(self.loop_edge(lp))
                || !walk.face_ok(self, self.loop_face(lp))
            {
                lp = opposite(start).0;
            }
            let ok = self.face_len(self.loop_face(lp)).is_multiple_of(2)
                && check(self.loop_edge(lp))
                && !visited.contains(self.loop_edge(lp));
            (lp, ok)
        };
        if ok {
            let next = self.loop_edge(lp);
            states.add(RingState {
                lp: Some(lp),
                wire: state.wire,
                no_calc: self.ring_delimited(next, walk.delimit),
            });
            visited.insert(next);
        }
        Some(edge)
    }

    #[must_use]
    pub fn walk_boundary(&self, start: EdgeId, walk: &Walk) -> Vec<EdgeId> {
        if !self.is_edge_boundary(start) {
            return Vec::new();
        }
        let mut visited: IdSet<EdgeId> = IdSet::new();
        let mut states = States::new(false);
        states.add(start);
        visited.insert(start);
        let mut out = Vec::new();
        while let Some(edge) = states.remove() {
            out.push(edge);
            if !walk.edge_ok(self, edge) {
                continue;
            }
            for vert in self.edge_verts(edge) {
                let others: Vec<EdgeId> = self.vert_edges(vert).collect();
                for other in others {
                    if other != edge
                        && self.is_edge_boundary(other)
                        && !visited.contains(other)
                        && walk.edge_ok(self, other)
                    {
                        states.add(other);
                        visited.insert(other);
                    }
                }
            }
        }
        out
    }

    fn is_loop_manifold(&self, lp: LoopId) -> bool {
        let next = self.loop_radial_next(lp);
        next != lp && self.loop_radial_next(next) == lp
    }

    fn non_manifold_next_around(
        &self,
        lp: LoopId,
        vert: VertId,
        face_count: usize,
    ) -> Option<LoopId> {
        let mut lp = lp;
        let mut guard = 0;
        loop {
            lp = self.loop_other_edge_loop(lp, vert);
            if self.is_loop_manifold(lp) {
                lp = self.loop_radial_next(lp);
            } else if self.edge_face_count(self.loop_edge(lp)) == face_count {
                return Some(lp);
            } else {
                return None;
            }
            guard += 1;
            if guard > self.loop_capacity() {
                return None;
            }
        }
    }

    #[must_use]
    pub fn walk_non_manifold_loop(&self, start: EdgeId) -> Vec<EdgeId> {
        let Some(_) = self.edge_first_loop(start) else {
            return vec![start];
        };
        let [v1, _] = self.edge_verts(start);
        let mut visited: IdSet<EdgeId> = IdSet::new();
        let mut states = States::new(false);
        visited.insert(start);
        states.add(NonManifoldState {
            start,
            start_vert: v1,
            current: start,
            last: v1,
            face_count: self.edge_face_count(start),
        });
        let mut out = Vec::new();
        while let Some(state) = states.remove() {
            out.push(state.current);
            let mut found: Option<LoopId> = None;
            let mut pivot = state.last;
            for pass in 0..2 {
                let (edge, vert) = match pass {
                    0 => (
                        state.current,
                        self.edge_other_vert(state.current, state.last)
                            .unwrap_or(state.last),
                    ),
                    _ => (state.start, state.start_vert),
                };
                pivot = vert;
                let loops: Vec<LoopId> = self.edge_loops(edge).collect();
                for lp in loops {
                    let next = self.non_manifold_next_around(lp, vert, state.face_count);
                    if let Some(next) = next
                        && !visited.contains(self.loop_edge(next))
                    {
                        match found {
                            None => found = Some(next),
                            Some(current) if self.loop_edge(current) != self.loop_edge(next) => {
                                found = None;
                                break;
                            }
                            Some(_) => {}
                        }
                    }
                }
                if found.is_some() {
                    break;
                }
            }
            if let Some(next) = found {
                let edge = self.loop_edge(next);
                visited.insert(edge);
                states.add(NonManifoldState {
                    current: edge,
                    last: pivot,
                    ..state
                });
            }
        }
        out
    }

    #[must_use]
    pub fn walk_island(&self, start: FaceId, walk: &Walk, only_manifold: bool) -> Vec<FaceId> {
        if !walk.face_ok(self, start) {
            return Vec::new();
        }
        let mut visited: IdSet<FaceId> = IdSet::new();
        let mut states = States::new(true);
        states.add(start);
        visited.insert(start);
        let mut out = Vec::new();
        while let Some(face) = states.remove() {
            out.push(face);
            for lp in self.face_loops(face) {
                if !walk.edge_ok(self, self.loop_edge(lp)) {
                    continue;
                }
                if only_manifold && self.loop_radial_next(lp) != lp {
                    let count = 1 + self
                        .edge_loops(self.loop_edge(lp))
                        .filter(|&other| other != lp && walk.face_ok(self, self.loop_face(other)))
                        .take(2)
                        .count();
                    if count != 2 {
                        continue;
                    }
                }
                let mut radial = self.loop_radial_next(lp);
                while radial != lp {
                    let other = self.loop_face(radial);
                    if walk.face_ok(self, other) && other != face && !visited.contains(other) {
                        states.add(other);
                        visited.insert(other);
                        break;
                    }
                    radial = self.loop_radial_next(radial);
                }
            }
        }
        out
    }

    #[must_use]
    pub fn walk_connected_verts(&self, start: VertId, walk: &Walk) -> Vec<VertId> {
        if !walk.vert_ok(self, start) {
            return Vec::new();
        }
        let mut visited: IdSet<VertId> = IdSet::new();
        let mut states = States::new(true);
        states.add(start);
        visited.insert(start);
        let mut out = Vec::new();
        while let Some(vert) = states.remove() {
            out.push(vert);
            for edge in self.vert_edges(vert) {
                if !walk.edge_ok(self, edge) {
                    continue;
                }
                let Some(other) = self.edge_other_vert(edge, vert) else {
                    continue;
                };
                if walk.vert_ok(self, other) && visited.insert(other) {
                    states.add(other);
                }
            }
        }
        out
    }

    #[must_use]
    pub fn walk_uv_edge_loop(&self, start: LoopId, uv_map: &str, walk: &Walk) -> Vec<LoopId> {
        let Some(uvs) = self.loop_uvs(uv_map) else {
            return Vec::new();
        };
        let uv = |lp: LoopId| -> Vec2 { uvs[lp.index()] };
        let mut visited: IdSet<LoopId> = IdSet::new();
        let mut states = States::new(false);
        states.add(start);
        visited.insert(start);
        let mut out = Vec::new();
        while let Some(lp) = states.remove() {
            out.push(lp);
            if !walk.edge_ok(self, self.loop_edge(lp)) {
                continue;
            }
            for pivot in [lp, self.loop_next(lp)] {
                let pivot_vert = self.loop_vert(pivot);
                let around: Vec<LoopId> = self.vert_loops(pivot_vert).collect();
                for first in around {
                    let mut radial = first;
                    loop {
                        let usable = !visited.contains(radial)
                            && (self.loop_vert(radial) == pivot_vert
                                || walk.edge_ok(self, self.loop_edge(radial)));
                        if usable {
                            let other = match self.loop_vert(radial) != pivot_vert {
                                true => self.loop_next(radial),
                                false => radial,
                            };
                            if uv(other) == uv(pivot) {
                                states.add(radial);
                                visited.insert(radial);
                            }
                        }
                        radial = self.loop_radial_next(radial);
                        if radial == first {
                            break;
                        }
                    }
                    if walk.edges.is_some() {
                        let boundary = self.loop_prev(first);
                        if self.is_edge_boundary(self.loop_edge(boundary))
                            && walk.edge_ok(self, self.loop_edge(boundary))
                            && visited.insert(boundary)
                        {
                            states.add(boundary);
                        }
                    }
                }
            }
        }
        out
    }
}
