use std::cmp::Ordering;
use std::collections::BinaryHeap;

use glam::Vec3;
use rikka_model_core::{INFLUENCES, NodeId};
use rustc_hash::FxHashMap;

use crate::asset::{Anchor, Bend, BindTarget, Edge, EdgeKind, Particle, Piece, PieceKind, Tether};
use crate::bind::{Binding, bind};
use crate::figure::Figure;
use crate::frame::{Frame, particle_normals};
use crate::geometry::{Graph, Shell, TriangleTree, Union};
use crate::proxy::Proxy;
use crate::relief::Relief;
use crate::style::Style;

const DENSITY: f32 = 0.25;
const LIGHTEST: f32 = 1e-5;
const TOP_RING: f32 = 0.01;
const ISLANDS: usize = 4;
const FAR: f32 = 1.0;
const SEAM_REACH: f32 = 0.02;
const SHORTEST: f32 = 0.4;
const SEAM_BAND: f32 = 0.004;
const RELIEF: f32 = 2.0;
const DEEPEST_INSIDE: f32 = 0.05;

pub(crate) struct ConstraintContext<'a> {
    pub(crate) figure: &'a Figure<'a>,
    pub(crate) body: &'a Shell,
    pub(crate) body_normals: &'a [Vec3],
    pub(crate) body_triangles: &'a [[u32; 3]],
    pub(crate) root: NodeId,
}

pub(crate) fn build_constraints(
    context: &ConstraintContext<'_>,
    mut proxy: Proxy,
    style: &Style,
    mut piece: Piece,
) -> Option<Piece> {
    if proxy.triangles.is_empty() || proxy.len() < 3 {
        return None;
    }
    let mut edges = edges(&proxy);
    let shortest = mean_length(&edges) * SHORTEST;
    let bends = bends(&proxy.positions, &proxy.triangles);
    let mut graph = structure(&proxy, &edges);
    let seams = seams(&proxy, &graph);
    if !seams.is_empty() {
        edges.extend(seams);
        edges.sort_unstable_by_key(|edge| edge.ends);
        graph = structure(&proxy, &edges);
    }
    pin_loose_parts(&graph, &proxy.positions, &mut proxy.pinned);
    let reached = graph.reach(
        (0..proxy.len() as u32).filter(|particle| proxy.pinned[*particle as usize]),
        f32::INFINITY,
    );
    let reach: Vec<f32> = reached
        .distance
        .iter()
        .map(|distance| if distance.is_finite() { *distance } else { 0.0 })
        .collect();
    let deepest = reach.iter().copied().fold(0.0f32, f32::max);
    let depth: Vec<f32> = reach
        .iter()
        .map(|reach| if deepest > 0.0 { reach / deepest } else { 0.0 })
        .collect();
    let spacing = mean_length(&edges);
    let nearest_parent = match piece.kind {
        PieceKind::Sheet => shortest,
        PieceKind::Strands => 0.0,
    };
    let parents = parents(&proxy, &graph, &reach, nearest_parent);
    let tethers = tethers(&proxy, &graph);
    let normals = particle_normals(&proxy.positions, &proxy.triangles);
    let triangles = TriangleTree::new(
        proxy
            .triangles
            .iter()
            .map(|corners| corners.map(|corner| proxy.positions[corner as usize]))
            .collect(),
    );
    let binds = bind(
        context.figure,
        &proxy,
        &Binding {
            normals: &normals,
            depth: &depth,
            deepest,
            spacing,
            body: context.body,
            far: FAR,
            triangles: &triangles,
        },
    );
    let mut radius: Vec<f32> = proxy
        .radius
        .iter()
        .map(|radius| radius.max(style.thickness * 0.5))
        .collect();
    let mut reach_limit = vec![f32::MAX; proxy.len()];
    for edge in &edges {
        for end in edge.ends {
            reach_limit[end as usize] = reach_limit[end as usize].min(edge.rest * 0.5);
        }
    }
    let relief = RELIEF * spacing;
    let mut render_offset = vec![0.0f32; proxy.len()];
    let drawn = Relief::measure(context.figure, &proxy, &triangles, relief);
    for bound in &binds {
        let corners = match &bound.bind.target {
            BindTarget::Triangles(influences) => {
                proxy.triangles[influences[0].triangle as usize].to_vec()
            }
            BindTarget::Strand(strand) => {
                let strand_node = proxy.strand_nodes[strand.weights[0].strand_node as usize];
                vec![strand_node.origin, strand_node.side]
            }
        };
        for corner in corners {
            let slot = &mut radius[corner as usize];
            *slot = slot.max(bound.thickness.min(reach_limit[corner as usize]));
            let covered = &mut render_offset[corner as usize];
            *covered = covered.max(bound.lift.min(relief));
        }
    }
    piece.particles = (0..proxy.len())
        .map(|index| {
            let rest = proxy.positions[index];
            Particle {
                rest,
                anchor: anchor(context, &proxy, index),
                pinned: proxy.pinned[index],
                mass: (proxy.area[index] * DENSITY).max(LIGHTEST),
                depth: depth[index],
                reach: reach[index],
                parent: parents[index],
                radius: radius[index],
                clearance: clearance(context.body, rest),
                render_offset: render_offset[index],
                relief_height: drawn.relief_height[index],
                relief_depth: drawn.relief_depth[index],
                spread: proxy.spread[index],
            }
        })
        .collect();
    add_cross_links(&mut edges, &proxy);
    piece.edges = edges;
    piece.bends = bends;
    piece.tethers = tethers;
    piece.triangles = proxy.triangles;
    piece.strand_nodes = proxy.strand_nodes;
    piece.bindings = binds.into_iter().map(|bound| bound.bind).collect();
    Some(piece)
}

fn add_cross_links(edges: &mut Vec<Edge>, proxy: &Proxy) {
    let edge_index: FxHashMap<[u32; 2], usize> = edges
        .iter()
        .enumerate()
        .map(|(at, edge)| (edge.ends, at))
        .collect();
    for &(a, b) in &proxy.cross_links {
        match edge_index.get(&[a, b]) {
            Some(&at) => edges[at].kind = EdgeKind::Structural,
            None => edges.push(Edge {
                ends: [a, b],
                rest: proxy.positions[a as usize].distance(proxy.positions[b as usize]),
                kind: EdgeKind::Structural,
            }),
        }
    }
}

fn edges(proxy: &Proxy) -> Vec<Edge> {
    let mut kinds: FxHashMap<(u32, u32), EdgeKind> = FxHashMap::default();
    let mut order: Vec<(u32, u32)> = Vec::new();
    let mut add = |a: u32, b: u32, kind: EdgeKind| {
        if a == b {
            return;
        }
        let key = (a.min(b), a.max(b));
        if let std::collections::hash_map::Entry::Vacant(slot) = kinds.entry(key) {
            slot.insert(kind);
            order.push(key);
        }
    };
    for (a, b, kind) in &proxy.edges {
        add(*a, *b, *kind);
    }
    for triangle in &proxy.triangles {
        for side in 0..3 {
            add(
                triangle[side],
                triangle[(side + 1) % 3],
                EdgeKind::Structural,
            );
        }
    }
    order.sort_unstable();
    order
        .into_iter()
        .map(|(a, b)| Edge {
            ends: [a, b],
            rest: proxy.positions[a as usize].distance(proxy.positions[b as usize]),
            kind: kinds[&(a, b)],
        })
        .collect()
}

#[derive(Clone, Copy)]
struct EdgeWing {
    from: u32,
    to: u32,
    wing: u32,
}

pub(crate) fn bends(positions: &[Vec3], triangles: &[[u32; 3]]) -> Vec<Bend> {
    let mut sides: FxHashMap<(u32, u32), Vec<EdgeWing>> = FxHashMap::default();
    for triangle in triangles {
        for side in 0..3 {
            let edge_wing = EdgeWing {
                from: triangle[side],
                to: triangle[(side + 1) % 3],
                wing: triangle[(side + 2) % 3],
            };
            sides
                .entry((
                    edge_wing.from.min(edge_wing.to),
                    edge_wing.from.max(edge_wing.to),
                ))
                .or_default()
                .push(edge_wing);
        }
    }
    let mut keys: Vec<(u32, u32)> = sides.keys().copied().collect();
    keys.sort_unstable();
    keys.into_iter()
        .filter_map(|key| match sides[&key].as_slice() {
            [first, second] => {
                let mut bend = Bend {
                    hinge: [first.from, first.to],
                    wings: [first.wing, second.wing],
                    rest_angle: 0.0,
                };
                bend.rest_angle = bend.angle(positions);
                Some(bend)
            }
            _ => None,
        })
        .collect()
}

fn mean_length(edges: &[Edge]) -> f32 {
    let structural: Vec<f32> = edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Structural)
        .map(|edge| edge.rest)
        .collect();
    if structural.is_empty() {
        0.02
    } else {
        structural.iter().sum::<f32>() / structural.len() as f32
    }
}

fn structure(proxy: &Proxy, edges: &[Edge]) -> Graph {
    Graph::new(
        proxy.len(),
        edges
            .iter()
            .filter(|edge| edge.kind != EdgeKind::Lateral)
            .map(|edge| (edge.ends[0], edge.ends[1], edge.rest)),
    )
}

fn pinned_components(pinned: &[bool], labels: &[u32], count: usize) -> Vec<bool> {
    let mut anchored = vec![false; count];
    for (label, pinned) in labels.iter().zip(pinned) {
        anchored[*label as usize] |= *pinned;
    }
    anchored
}

fn seams(proxy: &Proxy, graph: &Graph) -> Vec<Edge> {
    let (labels, count) = graph.components();
    let mut anchored = pinned_components(&proxy.pinned, &labels, count);
    let mut seams = Vec::new();
    loop {
        let mut joined = false;
        for component in 0..count {
            if anchored[component] {
                continue;
            }
            let nearest: Vec<(u32, u32, f32)> = (0..proxy.len() as u32)
                .filter(|loose| labels[*loose as usize] as usize == component)
                .filter_map(|loose| {
                    let at = proxy.positions[loose as usize];
                    (0..proxy.len() as u32)
                        .filter(|other| anchored[labels[*other as usize] as usize])
                        .map(|other| (loose, other, at.distance(proxy.positions[other as usize])))
                        .min_by(|a, b| a.2.total_cmp(&b.2).then(a.1.cmp(&b.1)))
                })
                .collect();
            let closest = nearest
                .iter()
                .map(|(_, _, distance)| *distance)
                .fold(f32::MAX, f32::min);
            if closest > SEAM_REACH {
                continue;
            }
            seams.extend(
                nearest
                    .iter()
                    .filter(|(_, _, distance)| *distance <= closest + SEAM_BAND)
                    .map(|(loose, other, distance)| Edge {
                        ends: [(*loose).min(*other), (*loose).max(*other)],
                        rest: *distance,
                        kind: EdgeKind::Structural,
                    }),
            );
            anchored[component] = true;
            joined = true;
        }
        if !joined {
            return seams;
        }
    }
}

pub(crate) fn pin_loose_parts(graph: &Graph, positions: &[Vec3], pinned: &mut [bool]) {
    let (labels, count) = graph.components();
    let anchored = pinned_components(pinned, &labels, count);
    let mut top = vec![f32::MIN; count];
    for (label, position) in labels.iter().zip(positions) {
        top[*label as usize] = top[*label as usize].max(position.y);
    }
    for ((label, position), pinned) in labels.iter().zip(positions).zip(pinned) {
        if !anchored[*label as usize] && position.y >= top[*label as usize] - TOP_RING {
            *pinned = true;
        }
    }
}

fn parents(proxy: &Proxy, graph: &Graph, reach: &[f32], shortest: f32) -> Vec<Option<u32>> {
    let mut order: Vec<u32> = (0..proxy.len() as u32).collect();
    order.sort_by(|a, b| {
        reach[*a as usize]
            .total_cmp(&reach[*b as usize])
            .then(a.cmp(b))
    });
    let mut parents: Vec<Option<u32>> = vec![None; proxy.len()];
    for particle in order {
        if proxy.pinned[particle as usize] {
            continue;
        }
        let at = proxy.positions[particle as usize];
        let mut closer: Vec<u32> = graph
            .neighbours(particle)
            .map(|(next, _)| next)
            .filter(|next| reach[*next as usize] < reach[particle as usize])
            .collect();
        let far_enough = |next: &u32| proxy.positions[*next as usize].distance(at) >= shortest;
        if closer.iter().any(far_enough) {
            closer.retain(far_enough);
        }
        let distance = |other: u32| proxy.positions[other as usize].distance(at);
        let pinned = closer
            .iter()
            .copied()
            .filter(|next| proxy.pinned[*next as usize])
            .min_by(|a, b| distance(*a).total_cmp(&distance(*b)).then(a.cmp(b)));
        parents[particle as usize] = pinned.or_else(|| {
            closer
                .iter()
                .copied()
                .map(|next| {
                    let toward = (proxy.positions[next as usize] - at).normalize_or_zero();
                    let onward = parents[next as usize]
                        .map(|grand| {
                            (proxy.positions[grand as usize] - proxy.positions[next as usize])
                                .normalize_or_zero()
                        })
                        .unwrap_or(toward);
                    (-toward.dot(onward), distance(next), next)
                })
                .min_by(|a, b| {
                    a.0.total_cmp(&b.0)
                        .then(a.1.total_cmp(&b.1))
                        .then(a.2.cmp(&b.2))
                })
                .map(|(_, _, next)| next)
        });
    }
    parents
}

#[derive(Clone, Copy, PartialEq)]
struct FrontEntry {
    distance: f32,
    island: u32,
    particle: u32,
    source: u32,
}

impl Eq for FrontEntry {}

impl Ord for FrontEntry {
    fn cmp(&self, other: &FrontEntry) -> Ordering {
        other
            .distance
            .total_cmp(&self.distance)
            .then_with(|| other.island.cmp(&self.island))
            .then_with(|| other.particle.cmp(&self.particle))
            .then_with(|| other.source.cmp(&self.source))
    }
}

impl PartialOrd for FrontEntry {
    fn partial_cmp(&self, other: &FrontEntry) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn tethers(proxy: &Proxy, graph: &Graph) -> Vec<Tether> {
    let mut union = Union::new(proxy.len());
    for particle in 0..proxy.len() as u32 {
        if !proxy.pinned[particle as usize] {
            continue;
        }
        for (next, _) in graph.neighbours(particle) {
            if proxy.pinned[next as usize] {
                union.join(particle, next);
            }
        }
    }
    let (islands, _) = union.labels();
    let mut found: Vec<Vec<FrontEntry>> = vec![Vec::new(); proxy.len()];
    let mut heap: BinaryHeap<FrontEntry> = (0..proxy.len() as u32)
        .filter(|particle| proxy.pinned[*particle as usize])
        .map(|particle| FrontEntry {
            distance: 0.0,
            island: islands[particle as usize],
            particle,
            source: particle,
        })
        .collect();
    let open = |waves: &[FrontEntry], island: u32| {
        waves.len() < ISLANDS && waves.iter().all(|wave| wave.island != island)
    };
    while let Some(wave) = heap.pop() {
        if !open(&found[wave.particle as usize], wave.island) {
            continue;
        }
        found[wave.particle as usize].push(wave);
        for (next, length) in graph.neighbours(wave.particle) {
            if open(&found[next as usize], wave.island) {
                heap.push(FrontEntry {
                    distance: wave.distance + length,
                    particle: next,
                    ..wave
                });
            }
        }
    }
    let mut tethers = Vec::new();
    for (particle, waves) in found.into_iter().enumerate() {
        if proxy.pinned[particle] {
            continue;
        }
        for wave in waves {
            tethers.push(Tether {
                particle: particle as u32,
                anchor: wave.source,
                length: wave.distance,
            });
        }
    }
    tethers
}

fn clearance(body: &Shell, rest: Vec3) -> f32 {
    match body.signed(rest, FAR) {
        Some(signed) if signed.distance < -DEEPEST_INSIDE => -signed.distance,
        Some(signed) => signed.distance,
        None => FAR,
    }
}

fn anchor(context: &ConstraintContext<'_>, proxy: &Proxy, index: usize) -> Anchor {
    if let Some((joints, weights)) = proxy.weights[index].top() {
        return Anchor::Skinned { joints, weights };
    }
    let point = proxy.positions[index];
    match context.body.signed(point, f32::INFINITY) {
        Some(signed) => {
            let ids = context.body_triangles[signed.triangle as usize];
            let frame = Frame::from_triangle(
                context.body.corners(signed.triangle),
                ids.map(|corner| context.body_normals[corner as usize]),
                signed.uv,
            );
            Anchor::Surface {
                triangle: signed.triangle,
                uv: signed.uv,
                offset: frame.local_point(point),
            }
        }
        None => {
            let mut weights = [0.0; INFLUENCES];
            weights[0] = 1.0;
            Anchor::Skinned {
                joints: [context.root; INFLUENCES],
                weights,
            }
        }
    }
}
