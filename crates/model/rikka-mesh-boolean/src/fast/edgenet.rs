use std::f64::consts::TAU;

use glam::{DVec2, DVec3};
use rikka_mesh_edit::{EdgeId, EditMesh, FaceId, Flags, VertId};
use rustc_hash::FxHashMap;

use crate::error::Result;

const MAX_PINCH_FIXES: usize = 64;

#[derive(Clone, Copy, Debug)]
pub(crate) struct EdgeNetOptions {
    pub(crate) island_connect: bool,
    pub(crate) partial_connect: bool,
}

#[derive(Clone, Copy)]
struct Half {
    from: usize,
    to: usize,
}

struct Graph {
    verts: Vec<VertId>,
    coords: Vec<DVec2>,
    index: FxHashMap<VertId, usize>,
    boundary: Vec<usize>,
    wires: Vec<(usize, usize)>,
}

impl Graph {
    fn vertex(&mut self, mesh: &EditMesh, vert: VertId, basis: (DVec3, DVec3)) -> usize {
        if let Some(&index) = self.index.get(&vert) {
            return index;
        }
        let position = mesh.position(vert).as_dvec3();
        self.verts.push(vert);
        self.coords
            .push(DVec2::new(position.dot(basis.0), position.dot(basis.1)));
        self.index.insert(vert, self.verts.len() - 1);
        self.verts.len() - 1
    }

    fn undirected(&self) -> Vec<(usize, usize)> {
        let count = self.boundary.len();
        let mut edges: Vec<(usize, usize)> = (0..count)
            .map(|i| (self.boundary[i], self.boundary[(i + 1) % count]))
            .collect();
        edges.extend_from_slice(&self.wires);
        edges
    }

    fn prune_dangling(&mut self) {
        loop {
            let mut degree = vec![0usize; self.verts.len()];
            for &(a, b) in &self.wires {
                degree[a] += 1;
                degree[b] += 1;
            }
            let before = self.wires.len();
            let boundary = &self.boundary;
            self.wires.retain(|&(a, b)| {
                let dangling = |v: usize| degree[v] == 1 && !boundary.contains(&v);
                !(dangling(a) || dangling(b))
            });
            if self.wires.len() == before {
                break;
            }
        }
    }

    fn components(&self) -> Vec<usize> {
        let mut parent: Vec<usize> = (0..self.verts.len()).collect();
        fn find(parent: &mut [usize], mut x: usize) -> usize {
            while parent[x] != x {
                parent[x] = parent[parent[x]];
                x = parent[x];
            }
            x
        }
        for (a, b) in self.undirected() {
            let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
            if ra != rb {
                parent[ra.max(rb)] = ra.min(rb);
            }
        }
        (0..self.verts.len())
            .map(|v| find(&mut parent, v))
            .collect()
    }

    fn halves(&self) -> Vec<Half> {
        let count = self.boundary.len();
        let mut halves: Vec<Half> = (0..count)
            .map(|i| Half {
                from: self.boundary[i],
                to: self.boundary[(i + 1) % count],
            })
            .collect();
        for &(a, b) in &self.wires {
            halves.push(Half { from: a, to: b });
            halves.push(Half { from: b, to: a });
        }
        halves
    }

    fn angle(&self, from: usize, to: usize) -> f64 {
        let direction = self.coords[to] - self.coords[from];
        direction.y.atan2(direction.x)
    }

    fn trace(&self) -> Vec<Vec<usize>> {
        let halves = self.halves();
        let mut outgoing: Vec<Vec<usize>> = vec![Vec::new(); self.verts.len()];
        for (index, half) in halves.iter().enumerate() {
            outgoing[half.from].push(index);
        }
        let mut used = vec![false; halves.len()];
        let mut cycles = Vec::new();
        for start in 0..halves.len() {
            if used[start] {
                continue;
            }
            let mut cycle = Vec::new();
            let mut current = start;
            let mut closed = false;
            for _ in 0..=halves.len() {
                if used[current] {
                    closed = current == start;
                    break;
                }
                used[current] = true;
                let Half { from, to } = halves[current];
                cycle.push(from);
                let reverse = self.angle(to, from);
                let next = outgoing[to]
                    .iter()
                    .copied()
                    .filter(|&candidate| halves[candidate].to != from || outgoing[to].len() == 1)
                    .min_by(|&x, &y| {
                        let turn = |h: usize| {
                            let value = (reverse - self.angle(to, halves[h].to)).rem_euclid(TAU);
                            if value == 0.0 { TAU } else { value }
                        };
                        turn(x).total_cmp(&turn(y)).then(x.cmp(&y))
                    });
                match next {
                    Some(next) => current = next,
                    None => break,
                }
            }
            if closed && cycle.len() >= 3 && self.area(&cycle) > 0.0 {
                cycles.push(cycle);
            }
        }
        cycles
    }

    fn area(&self, cycle: &[usize]) -> f64 {
        (0..cycle.len())
            .map(|i| self.coords[cycle[i]].perp_dot(self.coords[cycle[(i + 1) % cycle.len()]]))
            .sum::<f64>()
            * 0.5
    }

    fn inside(&self, cycle: &[usize], point: DVec2) -> bool {
        let mut inside = false;
        for i in 0..cycle.len() {
            let (a, b) = (
                self.coords[cycle[i]],
                self.coords[cycle[(i + 1) % cycle.len()]],
            );
            if (a.y > point.y) != (b.y > point.y) {
                let x = a.x + (point.y - a.y) / (b.y - a.y) * (b.x - a.x);
                if point.x < x {
                    inside = !inside;
                }
            }
        }
        inside
    }

    fn visible(&self, from: usize, to: usize, region: &[usize]) -> bool {
        if from == to {
            return false;
        }
        let (p, q) = (self.coords[from], self.coords[to]);
        if !self.inside(region, (p + q) * 0.5) {
            return false;
        }
        for (a, b) in self.undirected() {
            if a == from || a == to || b == from || b == to {
                if (a == from && b == to) || (a == to && b == from) {
                    return false;
                }
                continue;
            }
            if segments_touch(p, q, self.coords[a], self.coords[b]) {
                return false;
            }
        }
        true
    }

    fn connect_islands(&mut self) {
        let components = self.components();
        let boundary_root = components[self.boundary[0]];
        let mut islands: FxHashMap<usize, Vec<usize>> = FxHashMap::default();
        for (vertex, &root) in components.iter().enumerate() {
            let has_edge = self.wires.iter().any(|&(a, b)| a == vertex || b == vertex);
            if root != boundary_root && has_edge {
                islands.entry(root).or_default().push(vertex);
            }
        }
        let lowest = |coords: &[DVec2], members: &[usize]| -> usize {
            members
                .iter()
                .copied()
                .min_by(|&a, &b| {
                    coords[a]
                        .x
                        .total_cmp(&coords[b].x)
                        .then(coords[a].y.total_cmp(&coords[b].y))
                })
                .unwrap_or(members[0])
        };
        let mut ordered: Vec<Vec<usize>> = islands.into_values().collect();
        ordered.sort_by(|a, b| {
            let (pa, pb) = (
                self.coords[lowest(&self.coords, a)],
                self.coords[lowest(&self.coords, b)],
            );
            pa.x.total_cmp(&pb.x).then(pa.y.total_cmp(&pb.y))
        });
        let mut connected: Vec<bool> = components
            .iter()
            .map(|&root| root == boundary_root)
            .collect();
        let boundary = self.boundary.clone();
        for members in ordered {
            let low = lowest(&self.coords, &members);
            let high = members
                .iter()
                .copied()
                .max_by(|&a, &b| {
                    self.coords[a]
                        .x
                        .total_cmp(&self.coords[b].x)
                        .then(self.coords[a].y.total_cmp(&self.coords[b].y))
                })
                .unwrap_or(low);
            let mut targets = Vec::new();
            for source in [low, high] {
                let mut candidates: Vec<usize> =
                    (0..self.verts.len()).filter(|&v| connected[v]).collect();
                candidates.sort_by(|&a, &b| {
                    let da = self.coords[a].distance_squared(self.coords[source]);
                    let db = self.coords[b].distance_squared(self.coords[source]);
                    da.total_cmp(&db).then(a.cmp(&b))
                });
                if let Some(target) = candidates.into_iter().find(|&target| {
                    !targets.contains(&(source, target)) && self.visible(source, target, &boundary)
                }) {
                    self.wires.push((source, target));
                    targets.push((source, target));
                }
            }
            for &member in &members {
                connected[member] = true;
            }
        }
    }

    fn fix_pinch(&mut self, cycle: &[usize]) -> bool {
        let count = cycle.len();
        for i in 0..count {
            let Some(offset) = (1..count).find(|&k| cycle[(i + k) % count] == cycle[i]) else {
                continue;
            };
            let first: Vec<usize> = (1..offset).map(|k| cycle[(i + k) % count]).collect();
            let second: Vec<usize> = (offset + 1..count)
                .map(|k| cycle[(i + k) % count])
                .collect();
            for &p in &first {
                for &q in &second {
                    if p != q && self.visible(p, q, cycle) {
                        self.wires.push((p, q));
                        return true;
                    }
                }
            }
        }
        false
    }
}

fn orientation(a: DVec2, b: DVec2, c: DVec2) -> f64 {
    (b - a).perp_dot(c - a)
}

fn segments_touch(p: DVec2, q: DVec2, a: DVec2, b: DVec2) -> bool {
    let (d1, d2) = (orientation(p, q, a), orientation(p, q, b));
    let (d3, d4) = (orientation(a, b, p), orientation(a, b, q));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    let on = |s: DVec2, e: DVec2, x: DVec2, d: f64| {
        d == 0.0
            && x.x >= s.x.min(e.x)
            && x.x <= s.x.max(e.x)
            && x.y >= s.y.min(e.y)
            && x.y <= s.y.max(e.y)
    };
    on(p, q, a, d1) || on(p, q, b, d2) || on(a, b, p, d3) || on(a, b, q, d4)
}

fn has_repeat(cycle: &[usize]) -> bool {
    let mut sorted = cycle.to_vec();
    sorted.sort_unstable();
    sorted.windows(2).any(|pair| pair[0] == pair[1])
}

pub(crate) fn split_face(
    mesh: &mut EditMesh,
    face: FaceId,
    wires: &[EdgeId],
    options: EdgeNetOptions,
) -> Result<Vec<FaceId>> {
    if wires.is_empty() || !mesh.contains_face(face) {
        return Ok(Vec::new());
    }
    let normal = mesh.calc_face_normal(face).as_dvec3();
    let (tangent, bitangent) = normal.any_orthonormal_pair();
    let basis = match tangent.cross(bitangent).dot(normal) < 0.0 {
        true => (bitangent, tangent),
        false => (tangent, bitangent),
    };
    let mut graph = Graph {
        verts: Vec::new(),
        coords: Vec::new(),
        index: FxHashMap::default(),
        boundary: Vec::new(),
        wires: Vec::new(),
    };
    for vert in mesh.face_verts_vec(face) {
        let index = graph.vertex(mesh, vert, basis);
        graph.boundary.push(index);
    }
    let face_edges = mesh.face_edges_vec(face);
    for &edge in wires {
        if !mesh.contains_edge(edge) || face_edges.contains(&edge) {
            continue;
        }
        let [a, b] = mesh.edge_verts(edge);
        let (ia, ib) = (graph.vertex(mesh, a, basis), graph.vertex(mesh, b, basis));
        let key = (ia.min(ib), ia.max(ib));
        if ia != ib
            && !graph
                .wires
                .iter()
                .any(|&(x, y)| (x.min(y), x.max(y)) == key)
        {
            graph.wires.push((ia, ib));
        }
    }
    graph.prune_dangling();
    if graph.wires.is_empty() {
        return Ok(Vec::new());
    }
    let components = graph.components();
    let boundary_root = components[graph.boundary[0]];
    if options.island_connect {
        graph.connect_islands();
    } else {
        graph.wires.retain(|&(a, _)| components[a] == boundary_root);
    }
    let mut cycles = graph.trace();
    if options.partial_connect {
        for _ in 0..MAX_PINCH_FIXES {
            let Some(pinched) = cycles.iter().find(|cycle| has_repeat(cycle)).cloned() else {
                break;
            };
            if !graph.fix_pinch(&pinched) {
                break;
            }
            cycles = graph.trace();
        }
    }
    cycles.retain(|cycle| !has_repeat(cycle));
    let original: Vec<usize> = graph.boundary.clone();
    let changed = cycles.iter().any(|cycle| !same_cycle(cycle, &original));
    if !changed {
        return Ok(Vec::new());
    }
    let sample = mesh.sample_face(face);
    let selected = mesh.is_face_selected(face);
    let face_normal = mesh.face_normal(face);
    let mut created = Vec::new();
    for cycle in cycles {
        let verts: Vec<VertId> = cycle.iter().map(|&v| graph.verts[v]).collect();
        if same_cycle(&cycle, &original) || mesh.face_exists(&verts).is_some() {
            continue;
        }
        let new_face = mesh.new_face(&verts).example(face).create()?;
        mesh.set_face_flag(new_face, Flags::SELECT, selected);
        mesh.set_face_normal(new_face, face_normal);
        mesh.interpolate_face_from_sample(new_face, &sample, false);
        created.push(new_face);
    }
    if !created.is_empty() {
        mesh.kill_face(face)?;
    }
    Ok(created)
}

fn same_cycle(a: &[usize], b: &[usize]) -> bool {
    a.len() == b.len()
        && (0..a.len()).any(|shift| (0..a.len()).all(|i| a[(i + shift) % a.len()] == b[i]))
}
