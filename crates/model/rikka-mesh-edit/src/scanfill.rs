use glam::{Vec2, Vec3};

use crate::math::{newell_cross, normalize_2d_length, normalize_with_length, ortho_basis};

const EPSILON: f32 = 0.00003;
const EPSILON_SQ: f32 = EPSILON * EPSILON;
const POLY_UNSET: u16 = u16::MAX;
const VERT_NEW: u8 = 0;
const VERT_AVAILABLE: u8 = 1;
const EDGE_NEW: u8 = 0;
const EDGE_INTERNAL: u8 = 2;
const POLY_NEW: u8 = 0;
const POLY_VALID: u8 = 1;
const NIL: usize = usize::MAX;

#[derive(Clone, Copy, Debug)]
struct Vert {
    co: Vec3,
    xy: Vec2,
    poly: u16,
    edge_count: u32,
    flag: u8,
    prev: usize,
    next: usize,
}

#[derive(Clone, Copy, Debug)]
struct Edge {
    v1: usize,
    v2: usize,
    poly: u16,
    flag: u8,
    prev: usize,
    next: usize,
}

#[derive(Clone, Copy, Debug)]
struct List {
    first: usize,
    last: usize,
}

impl List {
    const EMPTY: List = List {
        first: NIL,
        last: NIL,
    };

    fn is_empty(&self) -> bool {
        self.first == NIL
    }
}

#[derive(Clone, Copy, Debug)]
struct Poly {
    edges: u32,
    verts: u32,
    min: Vec2,
    max: Vec2,
    nr: u16,
    flag: u8,
}

#[derive(Clone, Copy, Debug)]
struct ScanVert {
    vert: usize,
    edges: List,
}

#[derive(Debug, Default)]
pub struct ScanFill {
    verts: Vec<Vert>,
    edges: Vec<Edge>,
    vert_list: Option<List>,
    edge_list: Option<List>,
    faces: Vec<[usize; 3]>,
}

trait Links {
    fn prev(&self, index: usize) -> usize;
    fn next(&self, index: usize) -> usize;
    fn set_prev(&mut self, index: usize, value: usize);
    fn set_next(&mut self, index: usize, value: usize);
}

struct EdgeLinks<'a>(&'a mut Vec<Edge>);
struct VertLinks<'a>(&'a mut Vec<Vert>);

impl Links for EdgeLinks<'_> {
    fn prev(&self, index: usize) -> usize {
        self.0[index].prev
    }
    fn next(&self, index: usize) -> usize {
        self.0[index].next
    }
    fn set_prev(&mut self, index: usize, value: usize) {
        self.0[index].prev = value;
    }
    fn set_next(&mut self, index: usize, value: usize) {
        self.0[index].next = value;
    }
}

impl Links for VertLinks<'_> {
    fn prev(&self, index: usize) -> usize {
        self.0[index].prev
    }
    fn next(&self, index: usize) -> usize {
        self.0[index].next
    }
    fn set_prev(&mut self, index: usize, value: usize) {
        self.0[index].prev = value;
    }
    fn set_next(&mut self, index: usize, value: usize) {
        self.0[index].next = value;
    }
}

fn push_back(links: &mut impl Links, list: &mut List, item: usize) {
    links.set_next(item, NIL);
    links.set_prev(item, list.last);
    if list.last != NIL {
        links.set_next(list.last, item);
    }
    if list.first == NIL {
        list.first = item;
    }
    list.last = item;
}

fn remove(links: &mut impl Links, list: &mut List, item: usize) {
    let (prev, next) = (links.prev(item), links.next(item));
    if next != NIL {
        links.set_prev(next, prev);
    }
    if prev != NIL {
        links.set_next(prev, next);
    }
    if list.last == item {
        list.last = prev;
    }
    if list.first == item {
        list.first = next;
    }
}

fn insert_before(links: &mut impl Links, list: &mut List, before: usize, item: usize) {
    if before == NIL {
        push_back(links, list, item);
        return;
    }
    if list.first == NIL {
        list.first = item;
        list.last = item;
        return;
    }
    links.set_next(item, before);
    let prev = links.prev(before);
    links.set_prev(item, prev);
    links.set_prev(before, item);
    if prev != NIL {
        links.set_next(prev, item);
    }
    if list.first == before {
        list.first = item;
    }
}

fn collect(links: &impl Links, list: &List) -> Vec<usize> {
    let mut items = Vec::new();
    let mut item = list.first;
    while item != NIL {
        items.push(item);
        item = links.next(item);
    }
    items
}

fn compare_scan(a: Vec2, b: Vec2) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if a.y < b.y {
        return Ordering::Greater;
    }
    if a.y > b.y {
        return Ordering::Less;
    }
    if a.x > b.x {
        return Ordering::Greater;
    }
    if a.x < b.x {
        return Ordering::Less;
    }
    Ordering::Equal
}

fn edge_side(v1: Vec2, v2: Vec2, v3: Vec2) -> bool {
    let inp = (v2.x - v1.x) * (v1.y - v3.y) + (v1.y - v2.y) * (v1.x - v3.x);
    if inp < 0.0 {
        return false;
    }
    if inp == 0.0 && (v1 == v3 || v2 == v3) {
        return false;
    }
    true
}

fn cos_angle(p1: Vec2, p2: Vec2, p3: Vec2) -> f32 {
    let (a, _) = normalize_2d_length(p2 - p1);
    let (b, _) = normalize_2d_length(p2 - p3);
    a.dot(b)
}

fn close_2d(a: Vec2, b: Vec2, limit: f32) -> bool {
    (a.x - b.x).abs() <= limit && (a.y - b.y).abs() <= limit
}

fn close_3d(a: Vec3, b: Vec3, limit: f32) -> bool {
    (a.x - b.x).abs() <= limit && (a.y - b.y).abs() <= limit && (a.z - b.z).abs() <= limit
}

fn distance_squared_to_line(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let direction = end - start;
    let denominator = direction.dot(direction);
    let closest = match denominator == 0.0 {
        true => start,
        false => start + direction * (direction.dot(point - start) / denominator),
    };
    closest.distance_squared(point)
}

impl ScanFill {
    #[must_use]
    pub fn new() -> ScanFill {
        ScanFill::default()
    }

    pub fn add_vert(&mut self, co: Vec3) -> usize {
        let index = self.verts.len();
        self.verts.push(Vert {
            co,
            xy: Vec2::ZERO,
            poly: POLY_UNSET,
            edge_count: 0,
            flag: VERT_NEW,
            prev: NIL,
            next: NIL,
        });
        let mut list = self.vert_list.take().unwrap_or(List::EMPTY);
        push_back(&mut VertLinks(&mut self.verts), &mut list, index);
        self.vert_list = Some(list);
        index
    }

    fn new_edge(&mut self, v1: usize, v2: usize) -> usize {
        let index = self.edges.len();
        self.edges.push(Edge {
            v1,
            v2,
            poly: POLY_UNSET,
            flag: EDGE_NEW,
            prev: NIL,
            next: NIL,
        });
        index
    }

    pub fn add_edge(&mut self, v1: usize, v2: usize) -> usize {
        let index = self.new_edge(v1, v2);
        let mut list = self.edge_list.take().unwrap_or(List::EMPTY);
        push_back(&mut EdgeLinks(&mut self.edges), &mut list, index);
        self.edge_list = Some(list);
        index
    }

    #[must_use]
    pub fn triangles(&self) -> &[[usize; 3]] {
        &self.faces
    }

    fn add_edge_to_scan_vert(&mut self, scan: &mut ScanVert, edge: usize) -> bool {
        if scan.edges.first == NIL {
            scan.edges = List::EMPTY;
            push_back(&mut EdgeLinks(&mut self.edges), &mut scan.edges, edge);
            return true;
        }
        let record = self.edges[edge];
        let (x, y) = (self.verts[record.v1].xy.x, self.verts[record.v1].xy.y);
        let v2 = self.verts[record.v2].xy;
        let mut fac1 = v2.y - y;
        fac1 = match fac1 == 0.0 {
            true => 1.0e10 * (v2.x - x),
            false => (x - v2.x) / fac1,
        };
        let mut current = scan.edges.first;
        while current != NIL {
            let other = self.edges[current];
            if other.v2 == record.v2 {
                return false;
            }
            let other_v2 = self.verts[other.v2].xy;
            let mut fac = other_v2.y - y;
            fac = match fac == 0.0 {
                true => 1.0e10 * (other_v2.x - x),
                false => (x - other_v2.x) / fac,
            };
            if fac > fac1 {
                break;
            }
            current = other.next;
        }
        insert_before(
            &mut EdgeLinks(&mut self.edges),
            &mut scan.edges,
            current,
            edge,
        );
        true
    }

    fn add_edge_to_scan_list(&mut self, scan: &mut [ScanVert], edge: usize) -> Option<usize> {
        let (v1, v2) = (self.edges[edge].v1, self.edges[edge].v2);
        let (xy1, xy2) = (self.verts[v1].xy, self.verts[v2].xy);
        let swap = match xy1.y == xy2.y {
            true => xy1.x > xy2.x,
            false => xy1.y < xy2.y,
        };
        if swap {
            self.edges[edge].v1 = v2;
            self.edges[edge].v2 = v1;
        }
        let key = self.verts[self.edges[edge].v1].xy;
        let (mut low, mut high) = (0usize, scan.len());
        let mut slot = None;
        while low < high {
            let middle = (low + high) / 2;
            match compare_scan(key, self.verts[scan[middle].vert].xy) {
                std::cmp::Ordering::Less => high = middle,
                std::cmp::Ordering::Greater => low = middle + 1,
                std::cmp::Ordering::Equal => {
                    slot = Some(middle);
                    break;
                }
            }
        }
        let found = slot?;
        let mut target = scan[found];
        let added = self.add_edge_to_scan_vert(&mut target, edge);
        scan[found] = target;
        match added {
            true => None,
            false => Some(found),
        }
    }

    fn test_vertex_near_edge(&mut self) {
        let vert_list = self.vert_list.unwrap_or(List::EMPTY);
        let verts = collect(&VertLinks(&mut self.verts), &vert_list);
        for vert in verts {
            if self.verts[vert].edge_count != 1 {
                continue;
            }
            let edge_list = self.edge_list.unwrap_or(List::EMPTY);
            let mut first = edge_list.first;
            while first != NIL && !(self.edges[first].v1 == vert || self.edges[first].v2 == vert) {
                first = self.edges[first].next;
            }
            if first == NIL {
                continue;
            }
            if self.edges[first].v1 == vert {
                self.edges[first].v1 = self.edges[first].v2;
                self.edges[first].v2 = vert;
            }
            let edges = collect(&EdgeLinks(&mut self.edges), &edge_list);
            for edge in edges {
                let record = self.edges[edge];
                if vert == record.v1 || vert == record.v2 || self.verts[vert].poly != record.poly {
                    continue;
                }
                let xy = self.verts[vert].xy;
                if close_2d(xy, self.verts[record.v1].xy, EPSILON) {
                    self.edges[first].v2 = record.v1;
                    self.verts[record.v1].edge_count += 1;
                    self.verts[vert].edge_count = 0;
                    break;
                }
                if close_2d(xy, self.verts[record.v2].xy, EPSILON) {
                    self.edges[first].v2 = record.v2;
                    self.verts[record.v2].edge_count += 1;
                    self.verts[vert].edge_count = 0;
                    break;
                }
                let (a, b) = (self.verts[record.v1].xy, self.verts[record.v2].xy);
                let inside = xy.x >= a.x.min(b.x)
                    && xy.x <= a.x.max(b.x)
                    && xy.y >= a.y.min(b.y)
                    && xy.y <= a.y.max(b.y);
                if inside && distance_squared_to_line(a, b, xy) < EPSILON_SQ {
                    let added = self.add_edge(record.v1, vert);
                    self.edges[added].poly = record.poly;
                    self.edges[edge].v1 = vert;
                    self.verts[vert].edge_count = 3;
                    break;
                }
            }
        }
    }

    fn fill_poly(&mut self, poly: &Poly, edges: &mut List, verts_list: &List) -> u32 {
        let nr = poly.nr;
        let mut scan: Vec<ScanVert> = Vec::with_capacity(poly.verts as usize);
        for vert in collect(&VertLinks(&mut self.verts), verts_list) {
            if self.verts[vert].poly == nr {
                self.verts[vert].flag = VERT_NEW;
                scan.push(ScanVert {
                    vert,
                    edges: List::EMPTY,
                });
            }
        }
        scan.sort_by(|a, b| compare_scan(self.verts[a.vert].xy, self.verts[b.vert].xy));
        let count = scan.len();
        for edge in collect(&EdgeLinks(&mut self.edges), edges) {
            remove(&mut EdgeLinks(&mut self.edges), edges, edge);
            if self.edges[edge].v1 != self.edges[edge].v2 {
                self.add_edge_to_scan_list(&mut scan, edge);
            }
        }
        let two_connected = poly.flag == POLY_NEW;
        let mut total = 0u32;
        let max_faces = 2 * count as u32;
        let mut a = 0;
        while a < count {
            let current_edges = collect(&EdgeLinks(&mut self.edges), &scan[a].edges);
            for edge in current_edges {
                let (v1, v2) = (self.edges[edge].v1, self.edges[edge].v2);
                if self.verts[v1].edge_count == 1 || self.verts[v2].edge_count == 1 {
                    let mut list = scan[a].edges;
                    remove(&mut EdgeLinks(&mut self.edges), &mut list, edge);
                    scan[a].edges = list;
                    push_back(&mut EdgeLinks(&mut self.edges), edges, edge);
                    if self.verts[v1].edge_count > 1 {
                        self.verts[v1].edge_count -= 1;
                    }
                    if self.verts[v2].edge_count > 1 {
                        self.verts[v2].edge_count -= 1;
                    }
                } else {
                    self.verts[v2].flag = VERT_AVAILABLE;
                }
            }
            let mut stop = false;
            while !scan[a].edges.is_empty() {
                let ed1 = scan[a].edges.first;
                let ed2 = self.edges[ed1].next;
                if total >= max_faces {
                    stop = true;
                    break;
                }
                if ed2 == NIL {
                    let mut list = scan[a].edges;
                    remove(&mut EdgeLinks(&mut self.edges), &mut list, ed1);
                    scan[a].edges = List::EMPTY;
                    push_back(&mut EdgeLinks(&mut self.edges), edges, ed1);
                    let (e_v1, e_v2) = (self.edges[ed1].v1, self.edges[ed1].v2);
                    self.verts[e_v2].flag = VERT_NEW;
                    self.verts[e_v1].edge_count = self.verts[e_v1].edge_count.saturating_sub(1);
                    self.verts[e_v2].edge_count = self.verts[e_v2].edge_count.saturating_sub(1);
                } else {
                    let v1 = self.edges[ed1].v2;
                    let v2 = self.edges[ed1].v1;
                    let v3 = self.edges[ed2].v2;
                    if v1 == v2 || v2 == v3 {
                        break;
                    }
                    let (p1, p2, p3) = (self.verts[v1].xy, self.verts[v2].xy, self.verts[v3].xy);
                    let min_y = p1.y.min(p3.y);
                    let mut best: Option<usize> = None;
                    let mut best_cos = -1.0f32;
                    let mut first_time = false;
                    for (b, candidate) in scan.iter().enumerate().skip(a + 1) {
                        let vert = candidate.vert;
                        if self.verts[vert].flag != VERT_NEW {
                            continue;
                        }
                        let xy = self.verts[vert].xy;
                        if xy.y <= min_y {
                            break;
                        }
                        if edge_side(p1, p2, xy) && edge_side(p2, p3, xy) && edge_side(p3, p1, xy) {
                            match best {
                                None => best = Some(b),
                                Some(current) => {
                                    if !first_time {
                                        best_cos =
                                            cos_angle(p2, p1, self.verts[scan[current].vert].xy);
                                        first_time = true;
                                    }
                                    let test = cos_angle(p2, p1, xy);
                                    if test > best_cos {
                                        best = Some(b);
                                        best_cos = test;
                                    }
                                }
                            }
                        }
                    }
                    if let Some(best) = best {
                        let target = scan[best].vert;
                        let ed3 = self.new_edge(v2, target);
                        let mut list = scan[a].edges;
                        insert_before(&mut EdgeLinks(&mut self.edges), &mut list, ed2, ed3);
                        scan[a].edges = list;
                        self.verts[target].flag = VERT_AVAILABLE;
                        self.edges[ed3].flag = EDGE_INTERNAL;
                        self.verts[v2].edge_count += 1;
                        self.verts[target].edge_count += 1;
                    } else {
                        self.faces.push([v1, v2, v3]);
                        total += 1;
                        let mut list = scan[a].edges;
                        remove(&mut EdgeLinks(&mut self.edges), &mut list, ed1);
                        scan[a].edges = list;
                        push_back(&mut EdgeLinks(&mut self.edges), edges, ed1);
                        self.verts[v1].flag = VERT_NEW;
                        self.verts[v2].edge_count = self.verts[v2].edge_count.saturating_sub(1);
                        self.verts[v1].edge_count = self.verts[v1].edge_count.saturating_sub(1);
                        if self.edges[ed2].flag == EDGE_NEW && two_connected {
                            let mut list = scan[a].edges;
                            remove(&mut EdgeLinks(&mut self.edges), &mut list, ed2);
                            scan[a].edges = list;
                            push_back(&mut EdgeLinks(&mut self.edges), edges, ed2);
                            let (e_v1, e_v2) = (self.edges[ed2].v1, self.edges[ed2].v2);
                            self.verts[e_v2].flag = VERT_NEW;
                            self.verts[e_v1].edge_count =
                                self.verts[e_v1].edge_count.saturating_sub(1);
                            self.verts[e_v2].edge_count =
                                self.verts[e_v2].edge_count.saturating_sub(1);
                        }
                        let ed3 = self.new_edge(v1, v3);
                        self.edges[ed3].flag = EDGE_INTERNAL;
                        self.verts[v1].edge_count += 1;
                        self.verts[v3].edge_count += 1;
                        if let Some(existing) = self.add_edge_to_scan_list(&mut scan, ed3) {
                            let (e_v1, e_v2) = (self.edges[ed3].v1, self.edges[ed3].v2);
                            self.verts[e_v1].edge_count =
                                self.verts[e_v1].edge_count.saturating_sub(1);
                            self.verts[e_v2].edge_count =
                                self.verts[e_v2].edge_count.saturating_sub(1);
                            let mut candidate = scan[existing].edges.first;
                            while candidate != NIL {
                                let record = self.edges[candidate];
                                if (record.v1 == v1 && record.v2 == v3)
                                    || (record.v1 == v3 && record.v2 == v1)
                                {
                                    if two_connected {
                                        let mut list = scan[existing].edges;
                                        remove(
                                            &mut EdgeLinks(&mut self.edges),
                                            &mut list,
                                            candidate,
                                        );
                                        scan[existing].edges = list;
                                        push_back(
                                            &mut EdgeLinks(&mut self.edges),
                                            edges,
                                            candidate,
                                        );
                                        self.verts[record.v1].edge_count =
                                            self.verts[record.v1].edge_count.saturating_sub(1);
                                        self.verts[record.v2].edge_count =
                                            self.verts[record.v2].edge_count.saturating_sub(1);
                                    }
                                    break;
                                }
                                candidate = record.next;
                            }
                        }
                    }
                }
                for edge in collect(&EdgeLinks(&mut self.edges), &scan[a].edges) {
                    let (v1, v2) = (self.edges[edge].v1, self.edges[edge].v2);
                    if self.verts[v1].edge_count < 2 || self.verts[v2].edge_count < 2 {
                        let mut list = scan[a].edges;
                        remove(&mut EdgeLinks(&mut self.edges), &mut list, edge);
                        scan[a].edges = list;
                        push_back(&mut EdgeLinks(&mut self.edges), edges, edge);
                        if self.verts[v1].edge_count > 1 {
                            self.verts[v1].edge_count -= 1;
                        }
                        if self.verts[v2].edge_count > 1 {
                            self.verts[v2].edge_count -= 1;
                        }
                    }
                }
            }
            if stop {
                break;
            }
            a += 1;
        }
        total
    }

    pub fn calc(&mut self, normal: Option<Vec3>) -> u32 {
        let mut vert_list = self.vert_list.take().unwrap_or(List::EMPTY);
        let mut edge_list = self.edge_list.take().unwrap_or(List::EMPTY);
        for edge in collect(&EdgeLinks(&mut self.edges), &edge_list) {
            let (v1, v2) = (self.edges[edge].v1, self.edges[edge].v2);
            self.verts[v1].flag = VERT_AVAILABLE;
            self.verts[v2].flag = VERT_AVAILABLE;
        }
        let all_verts = collect(&VertLinks(&mut self.verts), &vert_list);
        if !all_verts
            .iter()
            .any(|&vert| self.verts[vert].flag == VERT_AVAILABLE)
        {
            return 0;
        }
        let normal = match normal {
            Some(normal) => normal,
            None => {
                let mut sum = Vec3::ZERO;
                let mut previous = self.verts[vert_list.last].co;
                for &vert in &all_verts {
                    let co = self.verts[vert].co;
                    if !close_3d(previous, co, EPSILON) {
                        sum += newell_cross(previous, co);
                        previous = co;
                    }
                }
                sum
            }
        };
        let (normal, length) = normalize_with_length(normal);
        if length == 0.0 {
            return 0;
        }
        let (axis_x, axis_y) = ortho_basis(-normal);
        let mut poly: u16 = 0;
        for &vert in &all_verts {
            let co = self.verts[vert].co;
            self.verts[vert].xy = Vec2::new(axis_x.dot(co), axis_y.dot(co));
            if self.verts[vert].poly == POLY_UNSET {
                let mut toggle = 0u32;
                let mut ok = true;
                self.verts[vert].poly = poly;
                while ok {
                    ok = false;
                    toggle += 1;
                    let mut edges = collect(&EdgeLinks(&mut self.edges), &edge_list);
                    if toggle & 1 == 0 {
                        edges.reverse();
                    }
                    for edge in edges {
                        let record = self.edges[edge];
                        let (p1, p2) = (self.verts[record.v1].poly, self.verts[record.v2].poly);
                        if p1 == POLY_UNSET && p2 == poly {
                            self.verts[record.v1].poly = poly;
                            self.edges[edge].poly = poly;
                            ok = true;
                        } else if p2 == POLY_UNSET && p1 == poly {
                            self.verts[record.v2].poly = poly;
                            self.edges[edge].poly = poly;
                            ok = true;
                        } else if record.poly == POLY_UNSET && p1 == poly && p2 == poly {
                            self.edges[edge].poly = poly;
                            ok = true;
                        }
                    }
                }
                poly += 1;
            }
        }
        for edge in collect(&EdgeLinks(&mut self.edges), &edge_list) {
            let (v1, v2) = (self.edges[edge].v1, self.edges[edge].v2);
            let over = self.verts[v1].edge_count > 250;
            self.verts[v1].edge_count += 1;
            if over {
                return 0;
            }
            let over = self.verts[v2].edge_count > 250;
            self.verts[v2].edge_count += 1;
            if over {
                return 0;
            }
        }
        self.vert_list = Some(vert_list);
        self.edge_list = Some(edge_list);
        self.test_vertex_near_edge();
        vert_list = self.vert_list.take().unwrap_or(List::EMPTY);
        edge_list = self.edge_list.take().unwrap_or(List::EMPTY);
        let mut ok = true;
        let mut toggle = 0u32;
        while ok {
            ok = false;
            toggle += 1;
            let mut edges = collect(&EdgeLinks(&mut self.edges), &edge_list);
            if toggle & 1 == 0 {
                edges.reverse();
            }
            for edge in edges {
                let (v1, v2) = (self.edges[edge].v1, self.edges[edge].v2);
                if self.verts[v1].edge_count == 1 {
                    self.verts[v2].edge_count = self.verts[v2].edge_count.saturating_sub(1);
                    remove(&mut VertLinks(&mut self.verts), &mut vert_list, v1);
                    remove(&mut EdgeLinks(&mut self.edges), &mut edge_list, edge);
                    ok = true;
                } else if self.verts[v2].edge_count == 1 {
                    self.verts[v1].edge_count = self.verts[v1].edge_count.saturating_sub(1);
                    remove(&mut VertLinks(&mut self.verts), &mut vert_list, v2);
                    remove(&mut EdgeLinks(&mut self.edges), &mut edge_list, edge);
                    ok = true;
                }
            }
        }
        if edge_list.is_empty() {
            return 0;
        }
        let mut polys: Vec<Poly> = (0..poly)
            .map(|nr| Poly {
                edges: 0,
                verts: 0,
                min: Vec2::splat(1.0e20),
                max: Vec2::splat(-1.0e20),
                nr,
                flag: POLY_NEW,
            })
            .collect();
        for edge in collect(&EdgeLinks(&mut self.edges), &edge_list) {
            if let Some(entry) = polys.get_mut(self.edges[edge].poly as usize) {
                entry.edges += 1;
            }
        }
        for vert in collect(&VertLinks(&mut self.verts), &vert_list) {
            let record = self.verts[vert];
            if let Some(entry) = polys.get_mut(record.poly as usize) {
                entry.verts += 1;
                entry.min = Vec2::new(
                    if entry.min.x < record.xy.x {
                        entry.min.x
                    } else {
                        record.xy.x
                    },
                    if entry.min.y < record.xy.y {
                        entry.min.y
                    } else {
                        record.xy.y
                    },
                );
                entry.max = Vec2::new(
                    if entry.max.x > record.xy.x {
                        entry.max.x
                    } else {
                        record.xy.x
                    },
                    if entry.max.y > record.xy.y {
                        entry.max.y
                    } else {
                        record.xy.y
                    },
                );
                if record.edge_count > 2 {
                    entry.flag = POLY_VALID;
                }
            }
        }
        if poly > 1 {
            let count = poly as usize;
            let mut target: Vec<usize> = (0..count).collect();
            for index in 0..count {
                if target[index] != index {
                    continue;
                }
                fill_target(&polys, index, index, &mut target);
            }
            for index in 0..count {
                if target[index] != index {
                    let source = polys[index];
                    let destination_index = target[index];
                    for vert in collect(&VertLinks(&mut self.verts), &vert_list) {
                        if self.verts[vert].poly == source.nr {
                            self.verts[vert].poly = polys[destination_index].nr;
                        }
                    }
                    for edge in collect(&EdgeLinks(&mut self.edges), &edge_list) {
                        if self.edges[edge].poly == source.nr {
                            self.edges[edge].poly = polys[destination_index].nr;
                        }
                    }
                    let destination = &mut polys[destination_index];
                    destination.verts += source.verts;
                    destination.edges += source.edges;
                    destination.max = destination.max.max(source.max);
                    destination.min = destination.min.min(source.min);
                    destination.flag |= source.flag;
                    polys[index].verts = 0;
                    polys[index].edges = 0;
                }
            }
        }
        let mut total = 0;
        for entry in &polys {
            if entry.edges > 1 {
                let mut edges = List::EMPTY;
                for edge in collect(&EdgeLinks(&mut self.edges), &edge_list) {
                    if self.edges[edge].poly == entry.nr {
                        remove(&mut EdgeLinks(&mut self.edges), &mut edge_list, edge);
                        push_back(&mut EdgeLinks(&mut self.edges), &mut edges, edge);
                    }
                }
                let mut verts = List::EMPTY;
                for vert in collect(&VertLinks(&mut self.verts), &vert_list) {
                    if self.verts[vert].poly == entry.nr {
                        remove(&mut VertLinks(&mut self.verts), &mut vert_list, vert);
                        push_back(&mut VertLinks(&mut self.verts), &mut verts, vert);
                    }
                }
                total += self.fill_poly(entry, &mut edges, &verts);
                for vert in collect(&VertLinks(&mut self.verts), &verts) {
                    remove(&mut VertLinks(&mut self.verts), &mut verts, vert);
                    push_back(&mut VertLinks(&mut self.verts), &mut vert_list, vert);
                }
                for edge in collect(&EdgeLinks(&mut self.edges), &edges) {
                    remove(&mut EdgeLinks(&mut self.edges), &mut edges, edge);
                    push_back(&mut EdgeLinks(&mut self.edges), &mut edge_list, edge);
                }
            }
        }
        self.vert_list = Some(vert_list);
        self.edge_list = Some(edge_list);
        total
    }
}

fn bounds_intersect(a: &Poly, b: &Poly) -> bool {
    if b.edges == 0 || a.edges == 0 {
        return false;
    }
    !(a.max.x < b.min.x || a.max.y < b.min.y || a.min.x > b.max.x || a.min.y > b.max.y)
}

fn fill_target(polys: &[Poly], target_index: usize, test: usize, target: &mut [usize]) {
    for other in target_index + 1..polys.len() {
        if target[other] != other {
            continue;
        }
        if bounds_intersect(&polys[test], &polys[other]) {
            target[other] = target_index;
            fill_target(polys, target_index, other, target);
        }
    }
}
