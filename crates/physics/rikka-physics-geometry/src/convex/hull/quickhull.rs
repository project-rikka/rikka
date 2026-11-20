use rikka_physics_math::{Plane, Vec3};

use super::view::HullEdge;

const NONE: u32 = u32::MAX;
const MAX_HULL_VERTICES: usize = 255;

#[derive(Clone, Copy, Debug)]
struct Link {
    prev: u32,
    next: u32,
}

const UNLINKED: Link = Link {
    prev: NONE,
    next: NONE,
};

#[derive(Clone, Copy, Debug)]
struct QhVertex {
    conflict_face: u32,
    position: Vec3,
    final_index: u32,
    reachable: bool,
}

#[derive(Clone, Copy, Debug)]
struct QhEdge {
    prev: u32,
    next: u32,
    origin: u32,
    face: u32,
    twin: u32,
    final_index: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    Visible,
    Delete,
}

#[derive(Clone, Copy, Debug)]
struct QhFace {
    edge: u32,
    mark: Mark,
    area: f32,
    plane: Plane,
    centroid: Vec3,
    max_conflict_distance: f32,
    conflict_head: u32,
    max_conflict: u32,
    final_index: u32,
    flipped: bool,
}

#[derive(Clone, Copy, Debug)]
struct HorizonFrame {
    start_edge: u32,
    edge: u32,
    started: bool,
}

#[derive(Debug)]
pub(crate) struct Topology {
    pub(crate) points: Vec<Vec3>,
    pub(crate) vertex_edges: Vec<u16>,
    pub(crate) edges: Vec<HullEdge>,
    pub(crate) faces: Vec<u16>,
    pub(crate) planes: Vec<Plane>,
}

#[derive(Debug, Default)]
struct Builder {
    tolerance: f32,
    min_radius: f32,
    min_outside: f32,
    interior_point: Vec3,

    vertices: Vec<QhVertex>,
    vertex_links: Vec<Link>,
    edges: Vec<QhEdge>,
    edge_free: u32,
    faces: Vec<QhFace>,
    face_links: Vec<Link>,
    face_free: u32,

    orphaned: u32,
    vertex_list: u32,
    face_list: u32,

    horizon: Vec<u32>,
    cone: Vec<u32>,
    merged: Vec<u32>,
    stack: Vec<HorizonFrame>,

    final_vertex_count: usize,
    final_half_edge_count: usize,
    final_face_count: usize,
}

fn list_init(links: &mut [Link], head: u32) {
    links[head as usize] = Link {
        prev: head,
        next: head,
    };
}

fn list_contains(links: &[Link], node: u32) -> bool {
    let link = links[node as usize];
    link.prev != NONE && link.next != NONE
}

fn list_insert(links: &mut [Link], node: u32, before: u32) {
    let prev = links[before as usize].prev;
    links[node as usize] = Link { prev, next: before };
    links[prev as usize].next = node;
    links[before as usize].prev = node;
}

fn list_remove(links: &mut [Link], node: u32) {
    let Link { prev, next } = links[node as usize];
    links[prev as usize].next = next;
    links[next as usize].prev = prev;
    links[node as usize] = UNLINKED;
}

fn list_push_back(links: &mut [Link], head: u32, node: u32) {
    list_insert(links, node, head);
}

fn list_is_empty(links: &[Link], head: u32) -> bool {
    links[head as usize].next == head
}

impl Builder {
    fn sentinel_vertex(&mut self) -> u32 {
        let index = self.vertices.len() as u32;
        self.vertices.push(QhVertex {
            conflict_face: NONE,
            position: Vec3::ZERO,
            final_index: NONE,
            reachable: false,
        });
        self.vertex_links.push(UNLINKED);
        list_init(&mut self.vertex_links, index);
        index
    }

    fn new_vertex(&mut self, position: Vec3) -> u32 {
        let index = self.vertices.len() as u32;
        self.vertices.push(QhVertex {
            conflict_face: NONE,
            position,
            final_index: NONE,
            reachable: false,
        });
        self.vertex_links.push(UNLINKED);
        index
    }

    fn new_edge(&mut self) -> u32 {
        let edge = QhEdge {
            prev: NONE,
            next: NONE,
            origin: NONE,
            face: NONE,
            twin: NONE,
            final_index: NONE,
        };
        if self.edge_free != NONE {
            let index = self.edge_free;
            self.edge_free = self.edges[index as usize].next;
            self.edges[index as usize] = edge;
            index
        } else {
            self.edges.push(edge);
            (self.edges.len() - 1) as u32
        }
    }

    fn retire_edge(&mut self, edge: u32) {
        self.edges[edge as usize].next = self.edge_free;
        self.edge_free = edge;
    }

    fn position(&self, vertex: u32) -> Vec3 {
        self.vertices[vertex as usize].position
    }

    fn edge(&self, edge: u32) -> QhEdge {
        self.edges[edge as usize]
    }

    fn new_face(&mut self, v1: u32, v2: u32, v3: u32) -> u32 {
        let face = if self.face_free != NONE {
            let index = self.face_free;
            self.face_free = self.face_links[index as usize].next;
            index
        } else {
            let conflict_head = self.sentinel_vertex();
            self.faces.push(QhFace {
                edge: NONE,
                mark: Mark::Visible,
                area: 0.0,
                plane: Plane::default(),
                centroid: Vec3::ZERO,
                max_conflict_distance: 0.0,
                conflict_head,
                max_conflict: NONE,
                final_index: NONE,
                flipped: false,
            });
            self.face_links.push(UNLINKED);
            (self.faces.len() - 1) as u32
        };
        self.face_links[face as usize] = UNLINKED;

        let e1 = self.new_edge();
        let e2 = self.new_edge();
        let e3 = self.new_edge();

        let p1 = self.position(v1);
        let p2 = self.position(v2);
        let p3 = self.position(v3);
        let cross = (p2 - p1).cross(p3 - p1);
        let length = cross.length();
        let normal = if length > 0.0 {
            cross / length
        } else {
            Vec3::ZERO
        };
        let plane = Plane::new(normal, normal.dot(p1));

        let head = self.faces[face as usize].conflict_head;
        list_init(&mut self.vertex_links, head);
        let interior = self.interior_point;
        self.faces[face as usize] = QhFace {
            edge: e1,
            mark: Mark::Visible,
            area: 0.5 * length,
            plane,
            centroid: (p1 + p2 + p3) / 3.0,
            max_conflict_distance: 0.0,
            conflict_head: head,
            max_conflict: NONE,
            final_index: NONE,
            flipped: plane.separation(interior) > 0.0,
        };

        for (edge, prev, next, origin) in [(e1, e3, e2, v1), (e2, e1, e3, v2), (e3, e2, e1, v3)] {
            self.edges[edge as usize] = QhEdge {
                prev,
                next,
                origin,
                face,
                twin: NONE,
                final_index: NONE,
            };
        }
        face
    }

    fn retire_face(&mut self, face: u32) {
        if list_contains(&self.face_links, face) {
            list_remove(&mut self.face_links, face);
        }
        self.faces[face as usize].edge = NONE;
        self.face_links[face as usize].next = self.face_free;
        self.face_free = face;
    }

    fn is_edge_convex(&self, edge: u32, tolerance: f32) -> bool {
        let e = self.edge(edge);
        let twin_face = self.edge(e.twin).face;
        self.faces[e.face as usize]
            .plane
            .separation(self.faces[twin_face as usize].centroid)
            < -tolerance
    }

    fn is_edge_concave(&self, edge: u32, tolerance: f32) -> bool {
        let e = self.edge(edge);
        let twin_face = self.edge(e.twin).face;
        self.faces[e.face as usize]
            .plane
            .separation(self.faces[twin_face as usize].centroid)
            > tolerance
    }

    fn vertex_count_of_face(&self, face: u32) -> usize {
        let start = self.faces[face as usize].edge;
        let mut edge = start;
        let mut count = 0;
        loop {
            count += 1;
            edge = self.edge(edge).next;
            if edge == start {
                return count;
            }
        }
    }

    fn nth_edge(&self, face: u32, index: usize) -> u32 {
        let mut edge = self.faces[face as usize].edge;
        for _ in 0..index {
            edge = self.edge(edge).next;
        }
        edge
    }

    fn link_face(&mut self, face: u32, index: usize, twin: u32) {
        let edge = self.nth_edge(face, index);
        self.edges[edge as usize].twin = twin;
        self.edges[twin as usize].twin = edge;
    }

    fn link_faces(&mut self, face1: u32, index1: usize, face2: u32, index2: usize) {
        let edge1 = self.nth_edge(face1, index1);
        let edge2 = self.nth_edge(face2, index2);
        self.edges[edge1 as usize].twin = edge2;
        self.edges[edge2 as usize].twin = edge1;
    }

    fn newell_plane(&mut self, face: u32) {
        let start = self.faces[face as usize].edge;
        let origin = self.position(self.edge(start).origin);
        let mut count = 0;
        let mut centroid = Vec3::ZERO;
        let mut normal = Vec3::ZERO;
        let mut edge = start;
        loop {
            let e = self.edge(edge);
            let v1 = self.position(e.origin) - origin;
            let v2 = self.position(self.edge(e.twin).origin) - origin;
            count += 1;
            centroid += v1;
            normal.x += (v1.y - v2.y) * (v1.z + v2.z);
            normal.y += (v1.z - v2.z) * (v1.x + v2.x);
            normal.z += (v1.x - v2.x) * (v1.y + v2.y);
            edge = e.next;
            if edge == start {
                break;
            }
        }
        let centroid = centroid / count as f32 + origin;
        let length = normal.length();
        let normal = if length > 0.0 {
            normal / length
        } else {
            Vec3::ZERO
        };
        let data = &mut self.faces[face as usize];
        data.centroid = centroid;
        data.plane = Plane::from_normal_point(normal, centroid);
        data.area = 0.5 * length;
    }

    fn compute_tolerance(&mut self, points: &[Vec3]) {
        let mut max_abs = Vec3::ZERO;
        for point in points {
            max_abs = max_abs.max(point.abs());
        }
        let max_sum = max_abs.x + max_abs.y + max_abs.z;
        let max_coordinate = max_abs.max_element();
        let max_distance = (3.0f32.sqrt() * max_coordinate).min(max_sum);
        self.tolerance = (3.0 * max_distance * 1.01 + max_coordinate) * f32::EPSILON;
        self.min_radius = 4.0 * self.tolerance;
        self.min_outside = 2.0 * self.min_radius;
    }

    fn build_initial_hull(&mut self, points: &[Vec3]) -> bool {
        let Some((index1, index2)) = farthest_along_axes(self.tolerance, points) else {
            return false;
        };
        let Some(index3) = farthest_from_line(index1, index2, self.tolerance, points) else {
            return false;
        };
        let Some(index4) = farthest_from_plane(index1, index2, index3, self.tolerance, points)
        else {
            return false;
        };
        let (index2, index3) = {
            let v1 = points[index1] - points[index4];
            let v2 = points[index2] - points[index4];
            let v3 = points[index3] - points[index4];
            if v1.dot(v2.cross(v3)) < 0.0 {
                (index3, index2)
            } else {
                (index2, index3)
            }
        };

        self.interior_point =
            0.25 * (points[index1] + points[index2] + points[index3] + points[index4]);

        let mut hull_vertices = [0u32; 4];
        for (slot, index) in hull_vertices
            .iter_mut()
            .zip([index1, index2, index3, index4])
        {
            let vertex = self.new_vertex(points[index]);
            list_push_back(&mut self.vertex_links, self.vertex_list, vertex);
            *slot = vertex;
        }
        let [v1, v2, v3, v4] = hull_vertices;

        let face1 = self.new_face(v1, v2, v3);
        let face2 = self.new_face(v4, v2, v1);
        let face3 = self.new_face(v4, v3, v2);
        let face4 = self.new_face(v4, v1, v3);
        for face in [face1, face2, face3, face4] {
            list_push_back(&mut self.face_links, self.face_list, face);
        }

        self.link_faces(face1, 0, face2, 1);
        self.link_faces(face1, 1, face3, 1);
        self.link_faces(face1, 2, face4, 1);
        self.link_faces(face2, 0, face3, 2);
        self.link_faces(face3, 0, face4, 2);
        self.link_faces(face4, 0, face2, 2);

        for (index, &point) in points.iter().enumerate() {
            if [index1, index2, index3, index4].contains(&index) {
                continue;
            }
            let mut max_distance = self.min_outside;
            let mut max_face = NONE;
            let mut node = self.face_links[self.face_list as usize].next;
            while node != self.face_list {
                let distance = self.faces[node as usize].plane.separation(point);
                if distance > max_distance {
                    max_distance = distance;
                    max_face = node;
                }
                node = self.face_links[node as usize].next;
            }
            if max_face != NONE {
                let vertex = self.new_vertex(point);
                self.add_conflict(max_face, vertex, max_distance);
            }
        }
        true
    }

    fn add_conflict(&mut self, face: u32, vertex: u32, distance: f32) {
        self.vertices[vertex as usize].conflict_face = face;
        let head = self.faces[face as usize].conflict_head;
        list_push_back(&mut self.vertex_links, head, vertex);
        let data = &mut self.faces[face as usize];
        if distance > data.max_conflict_distance {
            data.max_conflict_distance = distance;
            data.max_conflict = vertex;
        }
    }

    fn recache_conflicts(&mut self, face: u32) {
        let head = self.faces[face as usize].conflict_head;
        let plane = self.faces[face as usize].plane;
        let mut max_vertex = NONE;
        let mut max_distance = self.min_outside;
        let mut node = self.vertex_links[head as usize].next;
        while node != head {
            let distance = plane.separation(self.position(node));
            if distance > max_distance {
                max_distance = distance;
                max_vertex = node;
            }
            node = self.vertex_links[node as usize].next;
        }
        let data = &mut self.faces[face as usize];
        data.max_conflict = max_vertex;
        data.max_conflict_distance = max_distance;
    }

    fn next_conflict_vertex(&self) -> u32 {
        let mut max_vertex = NONE;
        let mut max_distance = self.min_outside;
        let mut node = self.face_links[self.face_list as usize].next;
        while node != self.face_list {
            let face = &self.faces[node as usize];
            if face.max_conflict != NONE && face.max_conflict_distance > max_distance {
                max_distance = face.max_conflict_distance;
                max_vertex = face.max_conflict;
            }
            node = self.face_links[node as usize].next;
        }
        max_vertex
    }

    fn drain_conflict_list(&mut self, face: u32) {
        let head = self.faces[face as usize].conflict_head;
        let mut node = self.vertex_links[head as usize].next;
        while node != head {
            let next = self.vertex_links[node as usize].next;
            self.vertices[node as usize].conflict_face = NONE;
            list_remove(&mut self.vertex_links, node);
            list_push_back(&mut self.vertex_links, self.orphaned, node);
            node = next;
        }
    }

    fn enter_horizon_face(&mut self, face: u32, entry_edge: u32) {
        self.faces[face as usize].mark = Mark::Delete;
        self.drain_conflict_list(face);
        let (start_edge, edge) = if entry_edge != NONE {
            (entry_edge, self.edge(entry_edge).next)
        } else {
            let first = self.faces[face as usize].edge;
            (first, first)
        };
        self.stack.push(HorizonFrame {
            start_edge,
            edge,
            started: false,
        });
    }

    fn build_horizon(&mut self, apex: u32, seed: u32) {
        self.stack.clear();
        self.enter_horizon_face(seed, NONE);
        let apex_position = self.position(apex);
        while let Some(top) = self.stack.last_mut() {
            if top.started && top.edge == top.start_edge {
                self.stack.pop();
                continue;
            }
            top.started = true;
            let edge = top.edge;
            let e = self.edges[edge as usize];
            top.edge = e.next;
            let twin_face = self.edges[e.twin as usize].face;
            if self.faces[twin_face as usize].mark != Mark::Visible {
                continue;
            }
            let distance = self.faces[twin_face as usize]
                .plane
                .separation(apex_position);
            if distance > self.min_radius {
                self.enter_horizon_face(twin_face, e.twin);
            } else {
                self.horizon.push(edge);
            }
        }
    }

    fn build_cone(&mut self, apex: u32) {
        for index in 0..self.horizon.len() {
            let edge = self.horizon[index];
            let e = self.edge(edge);
            let twin_origin = self.edge(e.twin).origin;
            let face = self.new_face(apex, e.origin, twin_origin);
            self.cone.push(face);
            self.link_face(face, 1, e.twin);
        }
        let mut face1 = *self.cone.last().unwrap_or(&NONE);
        for index in 0..self.cone.len() {
            let face2 = self.cone[index];
            self.link_faces(face1, 2, face2, 0);
            face1 = face2;
        }
    }

    fn destroy_edges(&mut self, begin: u32, end: u32) {
        let mut edge = begin;
        while edge != end {
            let next = self.edge(edge).next;
            self.retire_edge(edge);
            edge = next;
        }
    }

    fn connect_edges(&mut self, prev: u32, next: u32) {
        let prev_twin_face = self.edge(self.edge(prev).twin).face;
        let next_twin_face = self.edge(self.edge(next).twin).face;
        if prev_twin_face != next_twin_face {
            self.edges[prev as usize].next = next;
            self.edges[next as usize].prev = prev;
            return;
        }

        let next_face = self.edge(next).face;
        if self.faces[next_face as usize].edge == next {
            self.faces[next_face as usize].edge = prev;
        }

        let twin;
        if self.vertex_count_of_face(prev_twin_face) == 3 {
            let dead0 = self.edge(prev).twin;
            let dead1 = self.edge(next).twin;
            let dead2 = self.edge(dead1).prev;
            twin = self.edge(dead2).twin;

            self.faces[prev_twin_face as usize].mark = Mark::Delete;
            self.merged.push(prev_twin_face);

            let after = self.edge(next).next;
            self.edges[prev as usize].next = after;
            self.edges[after as usize].prev = prev;
            self.edges[prev as usize].twin = twin;
            self.edges[twin as usize].twin = prev;

            let redundant = self.edge(next).origin;
            list_remove(&mut self.vertex_links, redundant);

            self.retire_edge(dead0);
            self.retire_edge(dead1);
            self.retire_edge(dead2);
        } else {
            twin = self.edge(next).twin;
            let prev_twin = self.edge(prev).twin;
            let twin_face = self.edge(twin).face;
            if self.faces[twin_face as usize].edge == prev_twin {
                self.faces[twin_face as usize].edge = twin;
            }
            let after_twin = self.edge(prev_twin).next;
            self.edges[twin as usize].next = after_twin;
            self.edges[after_twin as usize].prev = twin;
            self.retire_edge(prev_twin);

            let after = self.edge(next).next;
            self.edges[prev as usize].next = after;
            self.edges[after as usize].prev = prev;
            self.edges[prev as usize].twin = twin;
            self.edges[twin as usize].twin = prev;

            let redundant = self.edge(next).origin;
            list_remove(&mut self.vertex_links, redundant);
        }

        let twin_face = self.edge(twin).face;
        self.newell_plane(twin_face);
        self.recache_conflicts(twin_face);
    }

    fn absorb_faces(&mut self, face: u32) {
        for index in 0..self.merged.len() {
            let merged = self.merged[index];
            let head = self.faces[merged as usize].conflict_head;
            let mut node = self.vertex_links[head as usize].next;
            while node != head {
                let next = self.vertex_links[node as usize].next;
                list_remove(&mut self.vertex_links, node);
                let distance = self.faces[face as usize]
                    .plane
                    .separation(self.position(node));
                if distance > self.min_outside {
                    self.add_conflict(face, node, distance);
                } else {
                    list_push_back(&mut self.vertex_links, self.orphaned, node);
                    self.vertices[node as usize].conflict_face = NONE;
                }
                node = next;
            }
            self.retire_face(merged);
        }
    }

    fn connect_faces(&mut self, edge: u32) {
        let face = self.edge(edge).face;
        let twin = self.edge(edge).twin;
        let twin_face = self.edge(twin).face;

        let mut edge_prev = self.edge(edge).prev;
        let mut edge_next = self.edge(edge).next;
        let mut twin_prev = self.edge(twin).prev;
        let mut twin_next = self.edge(twin).next;

        while self.edge(self.edge(edge_prev).twin).face == twin_face {
            edge_prev = self.edge(edge_prev).prev;
            twin_next = self.edge(twin_next).next;
        }
        while self.edge(self.edge(edge_next).twin).face == twin_face {
            edge_next = self.edge(edge_next).next;
            twin_prev = self.edge(twin_prev).prev;
        }

        self.faces[face as usize].edge = edge_prev;

        self.merged.clear();
        self.merged.push(twin_face);
        self.faces[twin_face as usize].mark = Mark::Delete;
        self.faces[twin_face as usize].edge = NONE;

        let stop = self.edge(twin_prev).next;
        let mut absorbed = twin_next;
        while absorbed != stop {
            self.edges[absorbed as usize].face = face;
            absorbed = self.edge(absorbed).next;
        }

        self.destroy_edges(self.edge(edge_prev).next, edge_next);
        self.destroy_edges(self.edge(twin_prev).next, twin_next);

        self.connect_edges(edge_prev, twin_next);
        self.connect_edges(twin_prev, edge_next);

        self.newell_plane(face);
        self.recache_conflicts(face);
        self.absorb_faces(face);
    }

    fn merge_concave(&mut self, face: u32) -> bool {
        let start = self.faces[face as usize].edge;
        let mut edge = start;
        loop {
            let twin = self.edge(edge).twin;
            if self.is_edge_concave(edge, self.min_radius)
                || self.is_edge_concave(twin, self.min_radius)
            {
                self.connect_faces(edge);
                return true;
            }
            edge = self.edge(edge).next;
            if edge == start {
                return false;
            }
        }
    }

    fn merge_coplanar(&mut self, face: u32) -> bool {
        let start = self.faces[face as usize].edge;
        let mut edge = start;
        loop {
            let twin = self.edge(edge).twin;
            if !self.is_edge_convex(edge, self.min_radius)
                || !self.is_edge_convex(twin, self.min_radius)
            {
                self.connect_faces(edge);
                return true;
            }
            edge = self.edge(edge).next;
            if edge == start {
                return false;
            }
        }
    }

    fn merge_faces(&mut self) {
        for index in 0..self.cone.len() {
            let face = self.cone[index];
            if self.faces[face as usize].mark == Mark::Visible && self.faces[face as usize].flipped
            {
                self.faces[face as usize].flipped = false;
                let start = self.faces[face as usize].edge;
                let mut best_area = 0.0;
                let mut best_edge = NONE;
                let mut edge = start;
                loop {
                    let twin_face = self.edge(self.edge(edge).twin).face;
                    let area = self.faces[twin_face as usize].area;
                    if area > best_area {
                        best_area = area;
                        best_edge = edge;
                    }
                    edge = self.edge(edge).next;
                    if edge == start {
                        break;
                    }
                }
                if best_edge != NONE {
                    self.connect_faces(best_edge);
                }
            }
        }
        for index in 0..self.cone.len() {
            let face = self.cone[index];
            if self.faces[face as usize].mark == Mark::Visible {
                while self.merge_concave(face) {}
            }
        }
        for index in 0..self.cone.len() {
            let face = self.cone[index];
            if self.faces[face as usize].mark == Mark::Visible {
                while self.merge_coplanar(face) {}
            }
        }
    }

    fn resolve_vertices(&mut self) {
        let mut node = self.vertex_links[self.orphaned as usize].next;
        while node != self.orphaned {
            let next = self.vertex_links[node as usize].next;
            list_remove(&mut self.vertex_links, node);
            let position = self.position(node);
            let mut max_distance = self.min_outside;
            let mut max_face = NONE;
            for &face in &self.cone {
                let data = &self.faces[face as usize];
                if data.mark == Mark::Visible {
                    let distance = data.plane.separation(position);
                    if distance > max_distance {
                        max_distance = distance;
                        max_face = face;
                    }
                }
            }
            if max_face != NONE {
                self.add_conflict(max_face, node, max_distance);
            }
            node = next;
        }
    }

    fn resolve_faces(&mut self) {
        let mut node = self.face_links[self.face_list as usize].next;
        while node != self.face_list {
            let next = self.face_links[node as usize].next;
            if self.faces[node as usize].mark == Mark::Delete
                && list_contains(&self.face_links, node)
            {
                let start = self.faces[node as usize].edge;
                let mut edge = start;
                loop {
                    let after = self.edge(edge).next;
                    self.retire_edge(edge);
                    edge = after;
                    if edge == start {
                        break;
                    }
                }
                self.retire_face(node);
            }
            node = next;
        }
        for index in 0..self.cone.len() {
            let face = self.cone[index];
            if self.faces[face as usize].mark != Mark::Delete {
                list_push_back(&mut self.face_links, self.face_list, face);
            }
        }
    }

    fn add_vertex_to_hull(&mut self, vertex: u32) {
        let face = self.vertices[vertex as usize].conflict_face;
        self.vertices[vertex as usize].conflict_face = NONE;
        list_remove(&mut self.vertex_links, vertex);
        list_push_back(&mut self.vertex_links, self.vertex_list, vertex);

        self.horizon.clear();
        self.build_horizon(vertex, face);
        self.cone.clear();
        self.build_cone(vertex);
        self.merge_faces();
        self.resolve_vertices();
        self.resolve_faces();
    }

    fn clean_hull(&mut self, origin: Vec3) {
        let mut face_count = 0;
        let mut half_edge_count = 0;
        let mut node = self.face_links[self.face_list as usize].next;
        while node != self.face_list {
            let start = self.faces[node as usize].edge;
            let mut edge = start;
            loop {
                let origin_vertex = self.edge(edge).origin;
                self.vertices[origin_vertex as usize].reachable = true;
                half_edge_count += 1;
                edge = self.edge(edge).next;
                if edge == start {
                    break;
                }
            }
            let data = &mut self.faces[node as usize];
            data.plane.offset += data.plane.normal.dot(origin);
            data.centroid += origin;
            face_count += 1;
            node = self.face_links[node as usize].next;
        }

        let mut vertex_count = 0;
        let mut node = self.vertex_links[self.vertex_list as usize].next;
        while node != self.vertex_list {
            let next = self.vertex_links[node as usize].next;
            if self.vertices[node as usize].reachable {
                self.vertices[node as usize].position += origin;
                vertex_count += 1;
            } else {
                list_remove(&mut self.vertex_links, node);
            }
            node = next;
        }
        self.interior_point += origin;
        self.final_vertex_count = vertex_count;
        self.final_half_edge_count = half_edge_count;
        self.final_face_count = face_count;
    }

    fn has_hull(&self) -> bool {
        let v = self.final_vertex_count as isize;
        let e = (self.final_half_edge_count / 2) as isize;
        let f = self.final_face_count as isize;
        v - e + f == 2 && f >= 4
    }

    fn export(&mut self) -> Option<Topology> {
        let mut points = Vec::with_capacity(self.final_vertex_count);
        let mut node = self.vertex_links[self.vertex_list as usize].next;
        while node != self.vertex_list {
            self.vertices[node as usize].final_index = points.len() as u32;
            points.push(self.position(node));
            node = self.vertex_links[node as usize].next;
        }
        if points.len() > MAX_HULL_VERTICES {
            return None;
        }

        let mut face_order = Vec::with_capacity(self.final_face_count);
        let mut edge_order = Vec::with_capacity(self.final_half_edge_count);
        let mut node = self.face_links[self.face_list as usize].next;
        while node != self.face_list {
            self.faces[node as usize].final_index = face_order.len() as u32;
            face_order.push(node);
            let start = self.faces[node as usize].edge;
            let mut edge = start;
            loop {
                if self.edge(edge).final_index == NONE {
                    let twin = self.edge(edge).twin;
                    self.edges[edge as usize].final_index = edge_order.len() as u32;
                    edge_order.push(edge);
                    self.edges[twin as usize].final_index = edge_order.len() as u32;
                    edge_order.push(twin);
                }
                edge = self.edge(edge).next;
                if edge == start {
                    break;
                }
            }
            node = self.face_links[node as usize].next;
        }
        if edge_order.len() > u16::MAX as usize || face_order.len() > u16::MAX as usize {
            return None;
        }

        let mut vertex_edges = vec![0u16; points.len()];
        let edges = edge_order
            .iter()
            .enumerate()
            .map(|(index, &edge)| {
                let e = self.edge(edge);
                let origin = self.vertices[e.origin as usize].final_index;
                vertex_edges[origin as usize] = index as u16;
                HullEdge::new(
                    self.edge(e.next).final_index as u16,
                    self.edge(e.twin).final_index as u16,
                    self.faces[e.face as usize].final_index as u16,
                    origin as u8,
                )
            })
            .collect();
        let faces = face_order
            .iter()
            .map(|&face| self.edge(self.faces[face as usize].edge).final_index as u16)
            .collect();
        let planes = face_order
            .iter()
            .map(|&face| self.faces[face as usize].plane)
            .collect();

        Some(Topology {
            points,
            vertex_edges,
            edges,
            faces,
            planes,
        })
    }
}

fn farthest_along_axes(tolerance: f32, points: &[Vec3]) -> Option<(usize, usize)> {
    let v0 = points[0];
    let mut min_point = [v0; 3];
    let mut max_point = [v0; 3];
    let mut min_index = [0usize; 3];
    let mut max_index = [0usize; 3];
    for (index, &v) in points.iter().enumerate().skip(1) {
        for axis in 0..3 {
            if v[axis] < min_point[axis][axis] {
                min_point[axis] = v;
                min_index[axis] = index;
            } else if v[axis] > max_point[axis][axis] {
                max_point[axis] = v;
                max_index[axis] = index;
            }
        }
    }
    let distance = Vec3::new(
        max_point[0].x - min_point[0].x,
        max_point[1].y - min_point[1].y,
        max_point[2].z - min_point[2].z,
    );
    let axis = if distance.x >= distance.y && distance.x >= distance.z {
        0
    } else if distance.y >= distance.z {
        1
    } else {
        2
    };
    (distance[axis] > 2.0 * tolerance).then_some((min_index[axis], max_index[axis]))
}

fn farthest_from_line(
    index1: usize,
    index2: usize,
    tolerance: f32,
    points: &[Vec3],
) -> Option<usize> {
    let a = points[index1];
    let ab = points[index2] - a;
    let inverse = 1.0 / ab.dot(ab);
    let mut max_distance = 4.0 * tolerance * tolerance;
    let mut max_index = None;
    for (index, &point) in points.iter().enumerate() {
        if index == index1 || index == index2 {
            continue;
        }
        let cross = (point - a).cross(ab);
        let distance = cross.dot(cross) * inverse;
        if distance > max_distance {
            max_distance = distance;
            max_index = Some(index);
        }
    }
    max_index
}

fn farthest_from_plane(
    index1: usize,
    index2: usize,
    index3: usize,
    tolerance: f32,
    points: &[Vec3],
) -> Option<usize> {
    let a = points[index1];
    let normal = (points[index2] - a)
        .cross(points[index3] - a)
        .normalize_or_zero();
    let plane = Plane::from_normal_point(normal, a);
    let mut max_distance = 2.0 * tolerance;
    let mut max_index = None;
    for (index, &point) in points.iter().enumerate() {
        if index == index1 || index == index2 || index == index3 {
            continue;
        }
        let distance = plane.separation(point).abs();
        if distance > max_distance {
            max_distance = distance;
            max_index = Some(index);
        }
    }
    max_index
}

pub(crate) fn quickhull(points: &[Vec3], max_vertices: usize) -> Option<Topology> {
    if points.len() < 4 {
        return None;
    }
    let origin = points[0];
    let shifted: Vec<Vec3> = points.iter().map(|&point| point - origin).collect();

    let mut builder = Builder {
        edge_free: NONE,
        face_free: NONE,
        ..Builder::default()
    };
    builder.orphaned = builder.sentinel_vertex();
    builder.vertex_list = builder.sentinel_vertex();
    builder.faces.push(QhFace {
        edge: NONE,
        mark: Mark::Delete,
        area: 0.0,
        plane: Plane::default(),
        centroid: Vec3::ZERO,
        max_conflict_distance: 0.0,
        conflict_head: NONE,
        max_conflict: NONE,
        final_index: NONE,
        flipped: false,
    });
    builder.face_links.push(UNLINKED);
    builder.face_list = 0;
    list_init(&mut builder.face_links, 0);

    builder.compute_tolerance(&shifted);
    if !builder.build_initial_hull(&shifted) {
        return None;
    }

    let mut budget = max_vertices.clamp(4, MAX_HULL_VERTICES) - 4;
    let mut vertex = builder.next_conflict_vertex();
    while vertex != NONE && budget > 0 {
        builder.add_vertex_to_hull(vertex);
        vertex = builder.next_conflict_vertex();
        budget -= 1;
    }

    builder.clean_hull(origin);
    if !builder.has_hull() || list_is_empty(&builder.vertex_links, builder.vertex_list) {
        return None;
    }
    builder.export()
}
