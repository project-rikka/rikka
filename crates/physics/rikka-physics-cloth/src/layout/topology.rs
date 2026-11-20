use std::collections::BTreeMap;

use glam::Vec3;
use rikka_model_core::NodeId;
use rikka_physics_tailor::{Anchor, BindTarget, ClothAsset, EdgeKind, Piece, PieceKind};

use crate::layout::colouring::colour;
use crate::layout::records::NONE;

const GROUP_PARTICLES: usize = 768;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) first: u32,
    pub(crate) count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Group {
    pub(crate) layer: u8,
    pub(crate) inner_end: u32,
    pub(crate) particles: Span,
    pub(crate) triangles: Span,
    pub(crate) plan: u32,
    pub(crate) structural_colours: u32,
    pub(crate) stretch_colours: u32,
    pub(crate) bends: Span,
    pub(crate) shapes: Span,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Frame {
    pub(crate) slot: u32,
    pub(crate) centre: Vec3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SlotBases {
    pub(crate) bends: u32,
    pub(crate) shapes: u32,
    pub(crate) wraps: u32,
    pub(crate) total: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct Topology {
    pub(crate) order: Vec<(usize, usize)>,
    pub(crate) gpu: Vec<Vec<u32>>,
    pub(crate) groups: Vec<Group>,
    pub(crate) triangles: Vec<[u32; 4]>,
    pub(crate) triangle_base: Vec<u32>,
    pub(crate) incident_starts: Vec<u32>,
    pub(crate) incident: Vec<u32>,
    pub(crate) edges: Vec<(usize, usize)>,
    pub(crate) bends: Vec<(usize, usize)>,
    pub(crate) shapes: Vec<(usize, usize)>,
    pub(crate) tether_starts: Vec<u32>,
    pub(crate) tethers: Vec<(usize, usize)>,
    pub(crate) plan: Vec<u32>,
    pub(crate) bases: SlotBases,
    pub(crate) gather_starts: Vec<u32>,
    pub(crate) dests: Vec<u32>,
    pub(crate) shares: Vec<Shares>,
    pub(crate) components: Vec<u32>,
    pub(crate) root_distance: Vec<f32>,
    pub(crate) rest_normals: Vec<Vec3>,
    pub(crate) aero_areas: Vec<f32>,
    pub(crate) inverse_mass: Vec<f32>,
    pub(crate) slots: Vec<NodeId>,
    pub(crate) slot_of: BTreeMap<NodeId, u32>,
    pub(crate) reach: Vec<f32>,
    pub(crate) frames: Vec<Frame>,
    pub(crate) nodes: Vec<NodeId>,
    pub(crate) binds: Vec<(usize, usize)>,
    pub(crate) absorb_starts: Vec<u32>,
    pub(crate) absorb: Vec<[u32; 4]>,
    pub(crate) layers: Vec<u8>,
    pub(crate) layer_triangles: Vec<[u32; 4]>,
    pub(crate) sheets: Vec<bool>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Shares {
    pub(crate) bends: f32,
    pub(crate) shapes: f32,
    pub(crate) wraps: f32,
}

impl Topology {
    pub(crate) fn build(asset: &ClothAsset) -> Topology {
        let slot_of = slots(asset);
        let slots: Vec<NodeId> = slot_of.keys().copied().collect();
        let mut layers: Vec<u8> = asset.pieces.iter().map(|piece| piece.layer).collect();
        layers.sort_unstable();
        layers.dedup();
        let mut topology = Topology {
            order: Vec::new(),
            gpu: asset
                .pieces
                .iter()
                .map(|piece| vec![NONE; piece.particles.len()])
                .collect(),
            groups: Vec::new(),
            triangles: Vec::new(),
            triangle_base: vec![0; asset.pieces.len()],
            incident_starts: Vec::new(),
            incident: Vec::new(),
            edges: Vec::new(),
            bends: Vec::new(),
            shapes: Vec::new(),
            tether_starts: Vec::new(),
            tethers: Vec::new(),
            plan: Vec::new(),
            bases: SlotBases::default(),
            gather_starts: Vec::new(),
            dests: Vec::new(),
            shares: Vec::new(),
            components: Vec::new(),
            root_distance: Vec::new(),
            rest_normals: Vec::new(),
            aero_areas: Vec::new(),
            inverse_mass: Vec::new(),
            reach: reach(asset, &slot_of),
            frames: asset
                .pieces
                .iter()
                .map(|piece| frame(piece, asset, &slot_of))
                .collect(),
            nodes: Vec::new(),
            binds: Vec::new(),
            absorb_starts: Vec::new(),
            absorb: Vec::new(),
            layers: asset.pieces.iter().map(|piece| piece.layer).collect(),
            layer_triangles: Vec::new(),
            sheets: asset
                .pieces
                .iter()
                .map(|piece| piece.kind == PieceKind::Sheet)
                .collect(),
            slots,
            slot_of,
        };
        let mut ranges: BTreeMap<u8, [u32; 4]> = BTreeMap::new();
        for layer in layers {
            let first = topology.triangles.len() as u32;
            let inner_end = topology.order.len() as u32;
            for pack in packs(asset, layer) {
                topology.add_group(asset, layer, inner_end, &pack);
            }
            ranges.insert(layer, [first, topology.triangles.len() as u32, 0, 0]);
        }
        topology.layer_triangles = topology.layers.iter().map(|layer| ranges[layer]).collect();
        topology.finish(asset);
        topology
    }

    pub(crate) fn count(&self) -> u32 {
        self.order.len() as u32
    }

    fn add_group(&mut self, asset: &ClothAsset, layer: u8, inner_end: u32, pieces: &[usize]) {
        let first = self.order.len() as u32;
        let mut members: Vec<(u32, usize, usize)> = Vec::new();
        for &piece in pieces {
            let levels = levels(&asset.pieces[piece]);
            members.extend(
                levels
                    .iter()
                    .enumerate()
                    .map(|(particle, &level)| (level, piece, particle)),
            );
        }
        members.sort_unstable();
        for (at, &(_, piece, particle)) in members.iter().enumerate() {
            self.gpu[piece][particle] = first + at as u32;
            self.order.push((piece, particle));
        }
        let triangles = self.triangles.len() as u32;
        for &piece in pieces {
            self.triangle_base[piece] = self.triangles.len() as u32;
            let map = &self.gpu[piece];
            self.triangles
                .extend(asset.pieces[piece].triangles.iter().map(|corners| {
                    [
                        map[corners[0] as usize],
                        map[corners[1] as usize],
                        map[corners[2] as usize],
                        piece as u32,
                    ]
                }));
        }
        let (plan, structural_colours) =
            self.coloured_edges(asset, pieces, |kind| kind == EdgeKind::Structural);
        let (_, stretch_colours) = self.coloured_edges(asset, pieces, |_| true);
        let bends = self.bends.len() as u32;
        let shapes = self.shapes.len() as u32;
        for &piece in pieces {
            let described_piece = &asset.pieces[piece];
            let movable = |particle: u32| !described_piece.particles[particle as usize].pinned;
            for (index, bend) in described_piece.bends.iter().enumerate() {
                let ids = [bend.hinge[0], bend.hinge[1], bend.wings[0], bend.wings[1]];
                if ids.iter().any(|&id| movable(id)) {
                    self.bends.push((piece, index));
                }
            }
            for child in 0..described_piece.particles.len() as u32 {
                if ancestor_chain(described_piece, child)
                    .iter()
                    .any(|&id| movable(id))
                {
                    self.shapes.push((piece, child as usize));
                }
            }
        }
        let span = |from: u32, to: usize| Span {
            first: from,
            count: to as u32 - from,
        };
        self.groups.push(Group {
            layer,
            inner_end,
            particles: span(first, self.order.len()),
            triangles: span(triangles, self.triangles.len()),
            plan,
            structural_colours,
            stretch_colours,
            bends: span(bends, self.bends.len()),
            shapes: span(shapes, self.shapes.len()),
        });
    }

    fn coloured_edges(
        &mut self,
        asset: &ClothAsset,
        pieces: &[usize],
        include: impl Fn(EdgeKind) -> bool,
    ) -> (u32, u32) {
        let mut items: Vec<(usize, usize)> = Vec::new();
        let mut ends: Vec<Vec<u32>> = Vec::new();
        for &piece in pieces {
            let described_piece = &asset.pieces[piece];
            let map = &self.gpu[piece];
            for (index, edge) in described_piece.edges.iter().enumerate() {
                let movable: Vec<u32> = edge
                    .ends
                    .iter()
                    .filter(|&&end| !described_piece.particles[end as usize].pinned)
                    .map(|&end| map[end as usize])
                    .collect();
                if include(edge.kind) && !movable.is_empty() {
                    items.push((piece, index));
                    ends.push(movable);
                }
            }
        }
        let colours = colour(&ends, self.order.len());
        let count = colours.iter().map(|&c| c + 1).max().unwrap_or(0);
        let mut ranked: Vec<(u32, usize)> = colours.iter().copied().zip(0..).collect();
        ranked.sort_unstable();
        let at = self.plan.len() as u32;
        let mut start = self.edges.len() as u32;
        for colour in 0..count {
            let members = ranked.iter().filter(|(c, _)| *c == colour);
            self.edges.extend(members.map(|&(_, item)| items[item]));
            let end = self.edges.len() as u32;
            self.plan.extend_from_slice(&[start, end]);
            start = end;
        }
        (at, count)
    }

    fn finish(&mut self, asset: &ClothAsset) {
        let count = self.order.len();
        let mut incident: Vec<Vec<u32>> = vec![Vec::new(); count];
        for (index, corners) in self.triangles.iter().enumerate() {
            for &corner in &corners[..3] {
                incident[corner as usize].push(index as u32);
            }
        }
        self.incident_starts = csr(&incident, &mut self.incident);
        let mut tethers: Vec<Vec<(usize, usize)>> = vec![Vec::new(); count];
        for (piece, described_piece) in asset.pieces.iter().enumerate() {
            for (index, tether) in described_piece.tethers.iter().enumerate() {
                tethers[self.gpu[piece][tether.particle as usize] as usize].push((piece, index));
            }
        }
        self.tether_starts = vec![0];
        for list in tethers {
            self.tethers.extend(list);
            self.tether_starts.push(self.tethers.len() as u32);
        }
        let mean_masses: Vec<f32> = asset
            .pieces
            .iter()
            .map(|piece| {
                let total: f32 = piece.particles.iter().map(|particle| particle.mass).sum();
                total / piece.particles.len() as f32
            })
            .collect();
        let normals: Vec<Vec<Vec3>> = asset.pieces.iter().map(rest_normals).collect();
        let areas: Vec<Vec<f32>> = asset.pieces.iter().map(aero_areas).collect();
        for &(piece, particle) in &self.order {
            let described_piece = &asset.pieces[piece];
            let described = &described_piece.particles[particle];
            self.root_distance
                .push(root_distance(described_piece, particle));
            self.rest_normals.push(normals[piece][particle]);
            self.aero_areas.push(areas[piece][particle]);
            self.inverse_mass.push(if described.pinned {
                0.0
            } else {
                mean_masses[piece] / described.mass
            });
        }
        for (piece, described_piece) in asset.pieces.iter().enumerate() {
            self.binds
                .extend((0..described_piece.bindings.len()).map(|bind| (piece, bind)));
            self.nodes
                .extend(described_piece.bindings.iter().map(|bind| bind.node));
        }
        self.nodes.sort_unstable();
        self.nodes.dedup();
        self.components = vec![0; count];
        let mut next = 0u32;
        for (piece, described_piece) in asset.pieces.iter().enumerate() {
            let labels = sheets(described_piece);
            let mut renamed = std::collections::BTreeMap::new();
            for (particle, label) in labels.iter().enumerate() {
                let id = *renamed.entry(*label).or_insert_with(|| {
                    next += 1;
                    next - 1
                });
                self.components[self.gpu[piece][particle] as usize] = id;
            }
        }
        self.gathers(asset);
        self.absorbing(asset);
    }

    fn gathers(&mut self, asset: &ClothAsset) {
        let count = self.order.len();
        let mut bases = SlotBases::default();
        let mut next = 0u32;
        let mut take = |items: usize, per: u32| {
            let at = next;
            next += items as u32 * per;
            at
        };
        bases.bends = take(self.bends.len(), 4);
        bases.shapes = take(self.shapes.len(), 3);
        bases.wraps = take(self.triangles.len(), 3);
        bases.total = next;
        self.bases = bases;
        let mut lists: Vec<[Vec<u32>; 3]> = vec![Default::default(); count];
        for (item, &(piece, index)) in self.bends.iter().enumerate() {
            let bend = asset.pieces[piece].bends[index];
            let ids = [bend.hinge[0], bend.hinge[1], bend.wings[0], bend.wings[1]];
            for (k, id) in ids.iter().enumerate() {
                let particle = self.gpu[piece][*id as usize] as usize;
                lists[particle][0].push(bases.bends + 4 * item as u32 + k as u32);
            }
        }
        for (item, &(piece, child)) in self.shapes.iter().enumerate() {
            let chain = ancestor_chain(&asset.pieces[piece], child as u32);
            for (k, id) in chain.iter().enumerate() {
                let particle = self.gpu[piece][*id as usize] as usize;
                lists[particle][1].push(bases.shapes + 3 * item as u32 + k as u32);
            }
        }
        for (item, corners) in self.triangles.iter().enumerate() {
            for (k, corner) in corners[..3].iter().enumerate() {
                lists[*corner as usize][2].push(bases.wraps + 3 * item as u32 + k as u32);
            }
        }
        let share = |n: usize| 1.0 / n.max(1) as f32;
        self.shares = lists
            .iter()
            .map(|families| Shares {
                bends: share(families[0].len()),
                shapes: share(families[1].len()),
                wraps: share(families[2].len()),
            })
            .collect();
        let flat: Vec<Vec<u32>> = lists
            .into_iter()
            .map(|families| families.concat())
            .collect();
        let mut gathered = Vec::new();
        self.gather_starts = csr(&flat, &mut gathered);
        self.dests = vec![NONE; bases.total as usize];
        for (at, slot) in gathered.iter().enumerate() {
            self.dests[*slot as usize] = at as u32;
        }
    }

    pub(crate) fn dest(&self, slot: u32) -> u32 {
        self.dests[slot as usize]
    }

    pub(crate) fn bend_share(&self, ids: &[u32]) -> f32 {
        self.shared(ids, |shares| shares.bends)
    }

    pub(crate) fn shape_share(&self, ids: &[u32]) -> f32 {
        self.shared(ids, |shares| shares.shapes)
    }

    fn shared(&self, ids: &[u32], family: impl Fn(&Shares) -> f32) -> f32 {
        ids.iter()
            .filter(|&&id| id != NONE && self.inverse_mass[id as usize] > 0.0)
            .map(|&id| family(&self.shares[id as usize]))
            .fold(1.0, f32::min)
    }

    fn absorbing(&mut self, asset: &ClothAsset) {
        let mut lists: Vec<Vec<[u32; 4]>> = vec![Vec::new(); self.order.len()];
        for &(piece, index) in &self.binds {
            let described_piece = &asset.pieces[piece];
            let bind = described_piece.bindings[index];
            let node = self.nodes.binary_search(&bind.node).unwrap_or(0) as u32;
            for (particle, weight) in shares(described_piece, &bind.target) {
                if weight > 0.0 {
                    let particle = self.gpu[piece][particle as usize] as usize;
                    lists[particle].push([node, bind.vertex, weight.to_bits(), 0]);
                }
            }
        }
        self.absorb_starts = vec![0];
        for list in lists {
            self.absorb.extend(list);
            self.absorb_starts.push(self.absorb.len() as u32);
        }
    }
}

fn shares(piece: &Piece, hold: &BindTarget) -> Vec<(u32, f32)> {
    match hold {
        BindTarget::Triangles(influences) => {
            let total: f32 = influences.iter().map(|influence| influence.weight).sum();
            influences
                .iter()
                .filter(|influence| influence.weight > 0.0)
                .flat_map(|influence| {
                    let corners = piece.triangles[influence.triangle as usize];
                    let shares = [
                        1.0 - influence.uv.x - influence.uv.y,
                        influence.uv.x,
                        influence.uv.y,
                    ];
                    corners
                        .into_iter()
                        .zip(shares.map(|share| influence.weight / total * share))
                })
                .collect()
        }
        BindTarget::Strand(strand) => {
            let total: f32 = strand.weights.iter().map(|entry| entry.weight).sum();
            strand
                .weights
                .iter()
                .filter(|entry| entry.weight > 0.0)
                .flat_map(|entry| {
                    let strand_node = piece.strand_nodes[entry.strand_node as usize];
                    let half = entry.weight / total * 0.5;
                    [(strand_node.origin, half), (strand_node.side, half)]
                })
                .collect()
        }
    }
}

pub(crate) fn ancestor_chain(piece: &Piece, child: u32) -> Vec<u32> {
    let mut chain = vec![child];
    let mut at = child;
    while chain.len() < 3 {
        match piece.particles[at as usize].parent {
            Some(parent) => {
                chain.push(parent);
                at = parent;
            }
            None => break,
        }
    }
    match chain.len() {
        1 => Vec::new(),
        _ => chain,
    }
}

fn sheets(piece: &Piece) -> Vec<usize> {
    let mut labels: Vec<usize> = (0..piece.particles.len()).collect();
    fn root(labels: &mut [usize], mut at: usize) -> usize {
        while labels[at] != at {
            labels[at] = labels[labels[at]];
            at = labels[at];
        }
        at
    }
    for edge in &piece.edges {
        let a = root(&mut labels, edge.ends[0] as usize);
        let b = root(&mut labels, edge.ends[1] as usize);
        labels[a.max(b)] = a.min(b);
    }
    (0..labels.len()).map(|at| root(&mut labels, at)).collect()
}

fn csr(lists: &[Vec<u32>], flat: &mut Vec<u32>) -> Vec<u32> {
    let mut starts = Vec::with_capacity(lists.len() + 1);
    starts.push(0);
    for list in lists {
        flat.extend_from_slice(list);
        starts.push(flat.len() as u32);
    }
    starts
}

fn packs(asset: &ClothAsset, layer: u8) -> Vec<Vec<usize>> {
    let mut pieces: Vec<usize> = (0..asset.pieces.len())
        .filter(|&index| asset.pieces[index].layer == layer)
        .collect();
    pieces.sort_by_key(|&index| {
        (
            std::cmp::Reverse(asset.pieces[index].particles.len()),
            index,
        )
    });
    let mut packs: Vec<(usize, Vec<usize>)> = Vec::new();
    for piece in pieces {
        let count = asset.pieces[piece].particles.len();
        match packs
            .iter_mut()
            .find(|(filled, _)| filled + count <= GROUP_PARTICLES)
        {
            Some((filled, members)) => {
                *filled += count;
                members.push(piece);
            }
            None => packs.push((count, vec![piece])),
        }
    }
    let mut packs: Vec<Vec<usize>> = packs
        .into_iter()
        .map(|(_, mut members)| {
            members.sort_unstable();
            members
        })
        .collect();
    packs.sort_unstable();
    packs
}

fn levels(piece: &Piece) -> Vec<u32> {
    let mut levels = vec![u32::MAX; piece.particles.len()];
    for start in 0..piece.particles.len() {
        let mut chain = Vec::new();
        let mut at = start;
        while levels[at] == u32::MAX {
            chain.push(at);
            match piece.particles[at].parent {
                Some(parent) => at = parent as usize,
                None => break,
            }
        }
        let base = match levels[at] {
            u32::MAX => 0,
            live_counts => live_counts + 1,
        };
        for (level, &member) in (base..).zip(chain.iter().rev()) {
            levels[member] = level;
        }
    }
    levels
}

fn root_distance(piece: &Piece, particle: usize) -> f32 {
    let mut distance = 0.0;
    let mut at = particle;
    while let Some(parent) = piece.particles[at].parent {
        distance += piece.particles[at]
            .rest
            .distance(piece.particles[parent as usize].rest);
        at = parent as usize;
    }
    distance
}

fn aero_areas(piece: &Piece) -> Vec<f32> {
    let mut areas = vec![0.0; piece.particles.len()];
    for corners in &piece.triangles {
        let [a, b, c] = corners.map(|corner| piece.particles[corner as usize].rest);
        let doubled = (b - a).cross(c - a).length();
        for &corner in corners {
            areas[corner as usize] += doubled / 3.0;
        }
    }
    areas
}

fn rest_normals(piece: &Piece) -> Vec<Vec3> {
    let mut sums = vec![Vec3::ZERO; piece.particles.len()];
    for corners in &piece.triangles {
        let [a, b, c] = corners.map(|corner| piece.particles[corner as usize].rest);
        let face = (b - a).cross(c - a);
        for &corner in corners {
            sums[corner as usize] += face;
        }
    }
    sums.into_iter().map(Vec3::normalize_or_zero).collect()
}

fn slots(asset: &ClothAsset) -> BTreeMap<NodeId, u32> {
    let mut nodes = Vec::new();
    for piece in &asset.pieces {
        for particle in &piece.particles {
            if let Anchor::Skinned { joints, weights } = particle.anchor {
                nodes.extend(used(&joints, &weights));
            }
        }
    }
    for vertex in &asset.body.vertices {
        nodes.extend(used(&vertex.joints, &vertex.weights));
    }
    nodes.extend(asset.body.colliders.iter().map(|collider| collider.node));
    nodes.sort_unstable();
    nodes.dedup();
    nodes
        .into_iter()
        .enumerate()
        .map(|(slot, node)| (node, slot as u32))
        .collect()
}

fn used<'a>(joints: &'a [NodeId; 4], weights: &'a [f32; 4]) -> impl Iterator<Item = NodeId> + 'a {
    joints
        .iter()
        .zip(weights)
        .filter(|(_, weight)| **weight > 0.0)
        .map(|(joint, _)| *joint)
}

fn leaning(anchor: Anchor, asset: &ClothAsset) -> Vec<(NodeId, f32)> {
    match anchor {
        Anchor::Skinned { joints, weights } => joints.into_iter().zip(weights).collect(),
        Anchor::Surface { triangle, uv, .. } => {
            let corners = asset.body.triangles[triangle as usize];
            let shares = [1.0 - uv.x - uv.y, uv.x, uv.y];
            corners
                .iter()
                .zip(shares)
                .flat_map(|(&corner, share)| {
                    let vertex = &asset.body.vertices[corner as usize];
                    vertex
                        .joints
                        .into_iter()
                        .zip(vertex.weights.map(|weight| weight * share))
                })
                .collect()
        }
    }
}

fn frame(piece: &Piece, asset: &ClothAsset, slot_of: &BTreeMap<NodeId, u32>) -> Frame {
    let pinned: Vec<_> = piece
        .particles
        .iter()
        .filter(|particle| particle.pinned)
        .collect();
    let chosen = if pinned.is_empty() {
        piece.particles.iter().collect()
    } else {
        pinned
    };
    let mut totals: BTreeMap<u32, f32> = BTreeMap::new();
    for particle in &chosen {
        for (node, weight) in leaning(particle.anchor, asset) {
            if weight > 0.0 {
                *totals.entry(slot_of[&node]).or_default() += weight;
            }
        }
    }
    let slot = totals
        .iter()
        .fold((0, f32::NEG_INFINITY), |best, (&slot, &total)| {
            if total > best.1 { (slot, total) } else { best }
        })
        .0;
    let centre = chosen.iter().map(|particle| particle.rest).sum::<Vec3>() / chosen.len() as f32;
    Frame { slot, centre }
}

fn reach(asset: &ClothAsset, slot_of: &BTreeMap<NodeId, u32>) -> Vec<f32> {
    let mut reach = vec![0.0f32; slot_of.len()];
    let mut stretch = |node: NodeId, point: Vec3| {
        let origin = asset.rest_worlds[node.index()].w_axis.truncate();
        let slot = slot_of[&node] as usize;
        reach[slot] = reach[slot].max(point.distance(origin));
    };
    for piece in &asset.pieces {
        for particle in &piece.particles {
            if let Anchor::Skinned { joints, weights } = particle.anchor {
                for node in used(&joints, &weights) {
                    stretch(node, particle.rest);
                }
            }
        }
    }
    for vertex in &asset.body.vertices {
        for node in used(&vertex.joints, &vertex.weights) {
            stretch(node, vertex.rest);
        }
    }
    reach
}
