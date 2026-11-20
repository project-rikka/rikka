use glam::Vec3;

use crate::analysis::Head;
use crate::asset::{EdgeKind, MAX_STRAND_NODES, StrandNode};
use crate::cloth_settings::Role;
use crate::figure::Figure;
use crate::geometry::{Graph, PointGrid, Principal, Union, groups};
use crate::proxy::{NodeWeights, Proxy, UNMAPPED};

const SMALL_CLUMP: usize = 16;
const MAX_BANDS: f32 = 40.0;
const SHORTEST: f32 = 0.004;
const ROOT_RING: f32 = 0.01;
const TOP_RING: f32 = 0.015;
const NARROWEST: f32 = 0.003;
const THINNEST: f32 = 0.002;
const THICKEST: f32 = 0.05;
const FLAT: f32 = 2.0;
const SEED_SHARE: f32 = 0.75;
const LATERAL_REACH: f32 = 1.5;
const ROUND: f32 = 0.5;
const WIDEST_TUBE: f32 = 0.12;
const TIE_ALONG: f32 = 2.5;
const ROOT_JOIN: f32 = 0.02;
const CLUMP_CROSS_LINKS: usize = 4;
const INTER_CLUMP_CROSS_LINKS: usize = 2;
const MIN_CROSS_LINK_LENGTH: f32 = 0.01;
const MAX_CROSS_LINK_DEGREE: usize = 6;

#[derive(Clone, Copy, Debug)]
pub(crate) struct BandWidths {
    pub(crate) band: f32,
    pub(crate) lateral: f32,
}

impl BandWidths {
    pub(crate) fn from_band(band: f32) -> BandWidths {
        BandWidths {
            band,
            lateral: (band * 2.5).max(0.03),
        }
    }
}

#[derive(Clone, Debug)]
struct Node {
    centre: Vec3,
    band: u32,
    reach: f32,
    clusters: Vec<u32>,
    parent: Option<usize>,
    attached: bool,
}

struct Clump {
    positions: Vec<Vec3>,
    pinned: Vec<bool>,
    normals: Vec<Vec3>,
    cluster_of: Vec<u32>,
    graph: Graph,
    edge: f32,
}

pub(crate) fn strands(
    figure: &Figure<'_>,
    head: Option<Head>,
    members: &[u32],
    roles: &[Role],
    spreads: &[f32],
    band_widths: BandWidths,
) -> Option<Proxy> {
    let mut proxy = Proxy::default();
    let mut clump_of_guide: Vec<u32> = Vec::new();
    let mut reach_of_guide: Vec<f32> = Vec::new();
    let mut levels: Vec<Level> = Vec::new();
    let mut vertex_map: Vec<(u32, u32, NodeWeights)> = Vec::new();
    let is_member = |vertex: &u32| members.binary_search(vertex).is_ok();
    let local: Vec<[u32; 3]> = figure
        .triangles
        .iter()
        .map(|triangle| triangle.corners)
        .filter(|corners| corners.iter().all(is_member))
        .collect();
    let clumps = clumps(figure, members, &local);
    let mut owner = vec![u32::MAX; figure.vertices.len()];
    for (index, clump) in clumps.iter().enumerate() {
        for vertex in clump {
            owner[*vertex as usize] = index as u32;
        }
    }
    let mut by_clump: Vec<Vec<[u32; 3]>> = vec![Vec::new(); clumps.len()];
    for corners in &local {
        let first = owner[corners[0] as usize];
        if first != u32::MAX
            && corners
                .iter()
                .all(|corner| owner[*corner as usize] == first)
        {
            by_clump[first as usize].push(*corners);
        }
    }
    for (index, (clump, triangles)) in clumps.iter().zip(&by_clump).enumerate() {
        let built = ribbon(
            figure,
            head,
            clump,
            triangles,
            roles,
            band_widths,
            &mut proxy,
        );
        for ((member, target), node_weights) in
            clump.iter().zip(built.vertex_map).zip(built.node_weights)
        {
            vertex_map.push((*member, target, node_weights));
        }
        clump_of_guide.extend(std::iter::repeat_n(index as u32, built.guides.len()));
        levels.extend(
            built
                .bands
                .iter()
                .zip(&built.reaches)
                .map(|(band, reach)| Level {
                    clump: index as u32,
                    band: *band,
                    reach: *reach,
                    width: built.width,
                }),
        );
        reach_of_guide.extend(built.reaches);
    }
    if proxy.triangles.is_empty() {
        return None;
    }
    let guides: Vec<u32> = (0..proxy.positions.len() as u32).step_by(2).collect();
    let fallback = PointGrid::new(
        guides
            .iter()
            .map(|guide| (*guide, proxy.positions[*guide as usize])),
        0.03,
    );
    (proxy.vertex_map, proxy.node_weights) = vertex_map
        .into_iter()
        .map(|(member, target, node_weights)| match target {
            UNMAPPED => {
                let guide = fallback
                    .nearest(figure.vertices[member as usize].position, |guide| {
                        proxy.positions[guide as usize]
                    })
                    .unwrap_or(0);
                ((member, guide), vec![(guide / 2, 1.0)])
            }
            target => ((member, target), node_weights),
        })
        .unzip();
    proxy.cross_links = cross_links(&proxy, &levels);
    laterals(&mut proxy, &clump_of_guide, &reach_of_guide, band_widths);
    proxy.gather(figure, spreads);
    for guide in guides {
        let (guide, offset) = (guide as usize, guide as usize + 1);
        let half = proxy.area[guide] * 0.5;
        proxy.area[guide] = half;
        proxy.area[offset] = half;
        proxy.weights[offset] = proxy.weights[guide].clone();
        proxy.spread[offset] = proxy.spread[guide];
    }
    Some(proxy)
}

fn clumps(figure: &Figure<'_>, members: &[u32], local: &[[u32; 3]]) -> Vec<Vec<u32>> {
    let mut union = Union::new(members.len());
    for corners in local {
        let at = corners.map(|corner| members.binary_search(&corner).unwrap_or(0) as u32);
        union.join(at[0], at[1]);
        union.join(at[1], at[2]);
    }
    let (labels, count) = union.labels();
    let same = figure.same_distance();
    let grid = PointGrid::new(
        members
            .iter()
            .enumerate()
            .map(|(at, member)| (at as u32, figure.vertices[*member as usize].position)),
        same * 2.0,
    );
    let mut touching: Vec<(u32, u32)> = Vec::new();
    for (at, member) in members.iter().enumerate() {
        let position = figure.vertices[*member as usize].position;
        for other in grid.near(position, same) {
            let (mine, theirs) = (labels[at], labels[other as usize]);
            if mine != theirs
                && figure.vertices[members[other as usize] as usize]
                    .position
                    .distance(position)
                    <= same
            {
                touching.push((mine.min(theirs), mine.max(theirs)));
            }
        }
    }
    touching.sort_unstable();
    let mut sizes = vec![0usize; count];
    for label in &labels {
        sizes[*label as usize] += 1;
    }
    let mut layered = Union::new(count);
    for run in touching.chunk_by(|a, b| a == b) {
        let (a, b) = run[0];
        if run.len() >= sizes[a as usize].min(sizes[b as usize]) {
            layered.join(a, b);
        }
    }
    let (merged, merged_count) = layered.labels();
    let labels: Vec<u32> = labels.iter().map(|label| merged[*label as usize]).collect();
    let islands: Vec<Vec<u32>> = groups(&labels, merged_count)
        .into_iter()
        .map(|island| island.into_iter().map(|at| members[at as usize]).collect())
        .collect();
    let (mut big, small): (Vec<Vec<u32>>, Vec<Vec<u32>>) = islands
        .into_iter()
        .partition(|island| island.len() >= SMALL_CLUMP);
    if big.is_empty() {
        let mut all: Vec<u32> = small.into_iter().flatten().collect();
        all.sort_unstable();
        return if all.is_empty() {
            Vec::new()
        } else {
            vec![all]
        };
    }
    let owners: Vec<(u32, u32)> = big
        .iter()
        .enumerate()
        .flat_map(|(island, vertices)| vertices.iter().map(move |vertex| (island as u32, *vertex)))
        .collect();
    let grid = PointGrid::new(
        owners
            .iter()
            .enumerate()
            .map(|(at, (_, vertex))| (at as u32, figure.vertices[*vertex as usize].position)),
        0.02,
    );
    for island in small {
        let at = figure.vertices[island[0] as usize].position;
        let owner = grid
            .nearest(at, |index| {
                figure.vertices[owners[index as usize].1 as usize].position
            })
            .map_or(0, |index| owners[index as usize].0);
        big[owner as usize].extend(island);
    }
    for island in &mut big {
        island.sort_unstable();
    }
    big
}
fn welded(figure: &Figure<'_>, clump: &[u32], triangles: &[[u32; 3]], roles: &[Role]) -> Clump {
    let same = figure.same_distance();
    let grid = PointGrid::new(
        clump
            .iter()
            .enumerate()
            .map(|(at, vertex)| (at as u32, figure.vertices[*vertex as usize].position)),
        same * 2.0,
    );
    let mut union = Union::new(clump.len());
    for (at, vertex) in clump.iter().enumerate() {
        let position = figure.vertices[*vertex as usize].position;
        for other in grid.near(position, same) {
            if other as usize > at
                && figure.vertices[clump[other as usize] as usize]
                    .position
                    .distance(position)
                    <= same
            {
                union.join(at as u32, other);
            }
        }
    }
    let (cluster_of, count) = union.labels();
    let mut positions = vec![Vec3::ZERO; count];
    let mut normals = vec![Vec3::ZERO; count];
    let mut counts = vec![0u32; count];
    let mut pinned = vec![false; count];
    for (vertex, cluster) in clump.iter().zip(&cluster_of) {
        let rest = &figure.vertices[*vertex as usize];
        positions[*cluster as usize] += rest.position;
        normals[*cluster as usize] += rest.normal;
        counts[*cluster as usize] += 1;
        pinned[*cluster as usize] |= roles[*vertex as usize] == Role::Pinned;
    }
    for (position, count) in positions.iter_mut().zip(&counts) {
        *position /= (*count).max(1) as f32;
    }
    let mut edges = Vec::new();
    for corners in triangles {
        let local = corners.map(|corner| cluster_of[clump.binary_search(&corner).unwrap_or(0)]);
        for side in 0..3 {
            let (a, b) = (local[side], local[(side + 1) % 3]);
            edges.push((a, b, positions[a as usize].distance(positions[b as usize])));
        }
    }
    let mut lengths: Vec<f32> = edges
        .iter()
        .map(|(_, _, length)| *length)
        .filter(|length| *length > 0.0)
        .collect();
    lengths.sort_by(f32::total_cmp);
    Clump {
        graph: Graph::new(count, edges),
        edge: lengths.get(lengths.len() / 2).copied().unwrap_or(0.0),
        positions,
        pinned,
        normals,
        cluster_of,
    }
}

struct Built {
    vertex_map: Vec<u32>,
    node_weights: Vec<NodeWeights>,
    guides: Vec<u32>,
    reaches: Vec<f32>,
    bands: Vec<u32>,
    width: f32,
}

fn ribbon(
    figure: &Figure<'_>,
    head: Option<Head>,
    clump: &[u32],
    triangles: &[[u32; 3]],
    roles: &[Role],
    band_widths: BandWidths,
    proxy: &mut Proxy,
) -> Built {
    let shape = welded(figure, clump, triangles, roles);
    let count = shape.positions.len();
    let mut roots: Vec<u32> = (0..count as u32)
        .filter(|cluster| shape.pinned[*cluster as usize])
        .collect();
    if roots.is_empty() {
        roots = fallback_roots(&shape.positions, head);
    }
    let reached = shape.graph.reach(roots.iter().copied(), f32::INFINITY);
    let length = reached
        .distance
        .iter()
        .copied()
        .filter(|distance| distance.is_finite())
        .fold(0.0f32, f32::max);
    let mut built = Built {
        vertex_map: vec![UNMAPPED; clump.len()],
        node_weights: vec![Vec::new(); clump.len()],
        guides: Vec::new(),
        reaches: Vec::new(),
        bands: Vec::new(),
        width: 0.0,
    };
    if length < SHORTEST {
        return built;
    }
    let width = band_widths
        .band
        .max(length / MAX_BANDS)
        .max(shape.edge)
        .min(length * 0.5);
    built.width = width;
    let is_root = |cluster: u32| reached.distance[cluster as usize] == 0.0;
    let band: Vec<Option<u32>> = (0..count as u32)
        .map(|cluster| {
            let distance = reached.distance[cluster as usize];
            match (distance.is_finite(), is_root(cluster)) {
                (false, _) => None,
                (true, true) => shape
                    .graph
                    .neighbours(cluster)
                    .any(|(next, _)| !is_root(next))
                    .then_some(0),
                (true, false) => Some(1 + (distance / width) as u32),
            }
        })
        .collect();
    let mut nodes = band_nodes(&shape, &band, &reached.distance, band_widths.lateral);
    let node_of = {
        let mut node_of = vec![usize::MAX; count];
        for (index, node) in nodes.iter().enumerate() {
            for cluster in &node.clusters {
                node_of[*cluster as usize] = index;
            }
        }
        node_of
    };
    let mut links: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for cluster in 0..count as u32 {
        let from = node_of[cluster as usize];
        if from == usize::MAX {
            continue;
        }
        for (next, _) in shape.graph.neighbours(cluster) {
            let to = node_of[next as usize];
            if to != usize::MAX && to != from {
                links[from].push(to);
            }
        }
    }
    for linked in &mut links {
        linked.sort_unstable();
        linked.dedup();
    }
    grow_tree(&mut nodes, &links);
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (index, node) in nodes.iter().enumerate() {
        if let (true, Some(parent)) = (node.attached, node.parent) {
            children[parent].push(index);
        }
    }
    for (index, node) in nodes.iter_mut().enumerate() {
        if node.parent.is_none() && children[index].is_empty() {
            node.attached = false;
        }
    }
    let order = tree_order(&nodes, &children);
    let mut particle = vec![u32::MAX; nodes.len()];
    let mut sides = vec![Vec3::ZERO; nodes.len()];
    for &index in &order {
        let node = &nodes[index];
        let tangent = match node.parent {
            Some(parent) => node.centre - nodes[parent].centre,
            None => children[index]
                .iter()
                .map(|child| nodes[*child].centre - node.centre)
                .sum::<Vec3>(),
        }
        .normalize_or(Vec3::NEG_Y);
        let points: Vec<Vec3> = node
            .clusters
            .iter()
            .map(|cluster| shape.positions[*cluster as usize])
            .collect();
        let normal_sum: Vec3 = node
            .clusters
            .iter()
            .map(|cluster| shape.normals[*cluster as usize])
            .sum();
        let away = match head {
            Some(head) => node.centre - head.centre,
            None => Vec3::ZERO,
        };
        let outward = normal_sum
            .try_normalize()
            .filter(|normal| normal.cross(tangent).length() > 0.3)
            .or_else(|| away.try_normalize())
            .unwrap_or(tangent.any_orthonormal_vector());
        let principal = Principal::compute(&points);
        let side = match principal {
            Some(principal)
                if principal.half[0] >= FLAT * principal.half[1]
                    && principal.half[0] > NARROWEST =>
            {
                let major = principal.axes[0];
                (major - tangent * major.dot(tangent)).try_normalize()
            }
            _ => None,
        }
        .or_else(|| tangent.cross(outward).try_normalize())
        .unwrap_or(tangent.any_orthonormal_vector());
        let side = match node.parent {
            Some(parent) if side.dot(sides[parent]) < 0.0 => -side,
            Some(_) => side,
            None if side.cross(tangent).dot(outward) < 0.0 => -side,
            None => side,
        };
        sides[index] = side;
        let radius = (points
            .iter()
            .map(|point| point.distance_squared(node.centre))
            .sum::<f32>()
            / points.len().max(1) as f32)
            .sqrt()
            .clamp(THINNEST, THICKEST);
        let reach = (radius).clamp(NARROWEST, band_widths.lateral * 0.5);
        let guide = proxy.positions.len() as u32;
        particle[index] = guide;
        let pinned = node.band == 0;
        proxy.positions.push(node.centre);
        proxy.positions.push(node.centre + side * reach);
        proxy.pinned.extend([pinned, pinned]);
        proxy.radius.extend([radius, radius]);
        proxy.edges.push((guide, guide + 1, EdgeKind::Structural));
        built.guides.push(guide);
        built.reaches.push(node.reach);
        built.bands.push(node.band);
        if let Some(parent) = node.parent {
            let (pg, po) = (particle[parent], particle[parent] + 1);
            let (cg, co) = (guide, guide + 1);
            proxy.triangles.push([pg, po, co]);
            proxy.triangles.push([pg, co, cg]);
            proxy.edges.extend([
                (pg, cg, EdgeKind::Structural),
                (po, co, EdgeKind::Structural),
                (pg, co, EdgeKind::Shear),
                (po, cg, EdgeKind::Shear),
            ]);
        }
    }
    for (index, linked) in links.iter().enumerate() {
        for &other in linked {
            let (a, b) = (&nodes[index], &nodes[other]);
            let related = a.parent == Some(other) || b.parent == Some(index);
            if index < other
                && a.attached
                && b.attached
                && !related
                && a.band.abs_diff(b.band) <= 1
                && a.centre.distance(b.centre) >= MIN_CROSS_LINK_LENGTH
            {
                proxy
                    .edges
                    .push((particle[index], particle[other], EdgeKind::Lateral));
            }
        }
    }
    proxy
        .strand_nodes
        .extend(strand_nodes(&nodes, &order, &children, &particle));
    let strand_weights = StrandWeights {
        nodes: &nodes,
        order: &order,
        particle: &particle,
        along: width * TIE_ALONG,
        across: WIDEST_TUBE,
    };
    let mut cluster_weights: Vec<Option<NodeWeights>> = vec![None; count];
    for (at, cluster) in shape.cluster_of.iter().enumerate() {
        let node = node_of[*cluster as usize];
        let position = shape.positions[*cluster as usize];
        let node = if node != usize::MAX && nodes[node].attached {
            node
        } else {
            match order.iter().min_by(|a, b| {
                nodes[**a]
                    .centre
                    .distance(position)
                    .total_cmp(&nodes[**b].centre.distance(position))
                    .then(a.cmp(b))
            }) {
                Some(node) => *node,
                None => continue,
            }
        };
        built.vertex_map[at] = particle[node];
        let along = match reached.distance[*cluster as usize] {
            distance if distance.is_finite() => distance,
            _ => nodes[node].reach,
        };
        built.node_weights[at] = cluster_weights[*cluster as usize]
            .get_or_insert_with(|| strand_weights.node_weights(node, along, position))
            .clone();
    }
    built
}

fn strand_nodes(
    nodes: &[Node],
    order: &[usize],
    children: &[Vec<usize>],
    particle: &[u32],
) -> Vec<StrandNode> {
    let mut deepest: Vec<f32> = nodes.iter().map(|node| node.reach).collect();
    for &index in order.iter().rev() {
        if let Some(parent) = nodes[index].parent {
            deepest[parent] = deepest[parent].max(deepest[index]);
        }
    }
    order
        .iter()
        .map(|&index| {
            let guide = particle[index];
            let main = children[index]
                .iter()
                .copied()
                .max_by(|a, b| deepest[*a].total_cmp(&deepest[*b]).then(b.cmp(a)));
            StrandNode {
                origin: guide,
                side: guide + 1,
                along: [
                    nodes[index].parent.map_or(guide, |parent| particle[parent]),
                    main.map_or(guide, |child| particle[child]),
                ],
            }
        })
        .collect()
}

struct StrandWeights<'a> {
    nodes: &'a [Node],
    order: &'a [usize],
    particle: &'a [u32],
    along: f32,
    across: f32,
}

impl StrandWeights<'_> {
    fn node_weights(&self, own: usize, along: f32, at: Vec3) -> NodeWeights {
        let mut weighted: NodeWeights = self
            .order
            .iter()
            .filter_map(|node| {
                let other = &self.nodes[*node];
                let lengthwise = (along - other.reach).abs();
                let sideways = (at.distance_squared(other.centre) - lengthwise * lengthwise)
                    .max(0.0)
                    .sqrt();
                let weight = falloff(lengthwise / self.along) * falloff(sideways / self.across);
                (weight > 0.0).then(|| (self.particle[*node] / 2, weight))
            })
            .collect();
        weighted.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        weighted.truncate(MAX_STRAND_NODES);
        let total: f32 = weighted.iter().map(|(_, weight)| weight).sum();
        if total > 0.0 {
            weighted
                .into_iter()
                .map(|(strand_node, weight)| (strand_node, weight / total))
                .collect()
        } else {
            vec![(self.particle[own] / 2, 1.0)]
        }
    }
}

fn falloff(ratio: f32) -> f32 {
    (1.0 - ratio * ratio).max(0.0).powi(3)
}

fn fallback_roots(positions: &[Vec3], head: Option<Head>) -> Vec<u32> {
    let measure: Vec<f32> = match head {
        Some(head) => positions
            .iter()
            .map(|position| position.distance(head.centre))
            .collect(),
        None => positions.iter().map(|position| -position.y).collect(),
    };
    let ring = match head {
        Some(_) => ROOT_RING,
        None => TOP_RING,
    };
    let least = measure.iter().copied().fold(f32::MAX, f32::min);
    (0..positions.len() as u32)
        .filter(|index| measure[*index as usize] <= least + ring)
        .collect()
}

fn band_nodes(shape: &Clump, band: &[Option<u32>], reach: &[f32], lateral: f32) -> Vec<Node> {
    let count = shape.positions.len();
    let mut union = Union::new(count);
    for cluster in 0..count as u32 {
        let Some(own) = band[cluster as usize] else {
            continue;
        };
        for (next, _) in shape.graph.neighbours(cluster) {
            if band[next as usize] == Some(own) {
                union.join(cluster, next);
            }
        }
    }
    let (labels, groups_count) = union.labels();
    let mut nodes = Vec::new();
    for component in groups(&labels, groups_count) {
        let Some(own) = band[component[0] as usize] else {
            continue;
        };
        let points: Vec<Vec3> = component
            .iter()
            .map(|cluster| shape.positions[*cluster as usize])
            .collect();
        let seeds = seeds(&points, lateral);
        let mut owned: Vec<Vec<u32>> = vec![Vec::new(); seeds.len()];
        for (at, point) in points.iter().enumerate() {
            let owner = (0..seeds.len())
                .min_by(|a, b| {
                    point
                        .distance_squared(points[seeds[*a]])
                        .total_cmp(&point.distance_squared(points[seeds[*b]]))
                        .then(a.cmp(b))
                })
                .unwrap_or(0);
            owned[owner].push(component[at]);
        }
        for clusters in owned.into_iter().filter(|clusters| !clusters.is_empty()) {
            let centre = clusters
                .iter()
                .map(|cluster| shape.positions[*cluster as usize])
                .sum::<Vec3>()
                / clusters.len() as f32;
            let reach = clusters
                .iter()
                .map(|cluster| reach[*cluster as usize])
                .sum::<f32>()
                / clusters.len() as f32;
            nodes.push(Node {
                centre,
                band: own,
                reach,
                clusters,
                parent: None,
                attached: own == 0,
            });
        }
    }
    nodes
}

fn seeds(points: &[Vec3], lateral: f32) -> Vec<usize> {
    let centroid = points.iter().copied().sum::<Vec3>() / points.len() as f32;
    let first = (0..points.len())
        .max_by(|a, b| {
            points[*a]
                .distance_squared(centroid)
                .total_cmp(&points[*b].distance_squared(centroid))
                .then(b.cmp(a))
        })
        .unwrap_or(0);
    let mut seeds = vec![first];
    let round = Principal::compute(points).is_some_and(|principal| {
        principal.half[1] >= ROUND * principal.half[0] && principal.half[0] * 2.0 <= WIDEST_TUBE
    });
    if round {
        return seeds;
    }
    let mut gap: Vec<f32> = points
        .iter()
        .map(|point| point.distance(points[first]))
        .collect();
    loop {
        let (far, distance) = gap
            .iter()
            .copied()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
            .unwrap_or((0, 0.0));
        if distance <= lateral * SEED_SHARE {
            return seeds;
        }
        seeds.push(far);
        for (nearest, point) in gap.iter_mut().zip(points) {
            *nearest = nearest.min(point.distance(points[far]));
        }
    }
}

fn grow_tree(nodes: &mut [Node], links: &[Vec<usize>]) {
    let deepest = nodes.iter().map(|node| node.band).max().unwrap_or(0);
    for band in 1..=deepest {
        let members: Vec<usize> = (0..nodes.len())
            .filter(|index| nodes[*index].band == band)
            .collect();
        for &index in &members {
            let candidates: Vec<usize> = links[index]
                .iter()
                .copied()
                .filter(|other| nodes[*other].band < band && nodes[*other].attached)
                .collect();
            let highest = candidates.iter().map(|other| nodes[*other].band).max();
            let parent = straightest(
                nodes,
                index,
                candidates
                    .into_iter()
                    .filter(|other| Some(nodes[*other].band) == highest),
            );
            if let Some(parent) = parent {
                nodes[index].parent = Some(parent);
                nodes[index].attached = true;
            }
        }
        loop {
            let mut grown = false;
            for &index in &members {
                if nodes[index].attached {
                    continue;
                }
                let parent = straightest(
                    nodes,
                    index,
                    links[index]
                        .iter()
                        .copied()
                        .filter(|other| nodes[*other].band == band && nodes[*other].attached),
                );
                if let Some(parent) = parent {
                    nodes[index].parent = Some(parent);
                    nodes[index].attached = true;
                    grown = true;
                }
            }
            if !grown {
                break;
            }
        }
    }
}

fn straightest(
    nodes: &[Node],
    child: usize,
    candidates: impl Iterator<Item = usize>,
) -> Option<usize> {
    let at = nodes[child].centre;
    candidates
        .map(|candidate| {
            let candidate_node = &nodes[candidate];
            let toward = at - candidate_node.centre;
            let bend = match candidate_node.parent {
                Some(grand) => (candidate_node.centre - nodes[grand].centre)
                    .normalize_or_zero()
                    .dot(toward.normalize_or_zero()),
                None => 1.0,
            };
            (toward.length() * (2.0 - bend), candidate)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, candidate)| candidate)
}

fn tree_order(nodes: &[Node], children: &[Vec<usize>]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..nodes.len())
        .filter(|index| nodes[*index].attached && nodes[*index].parent.is_none())
        .collect();
    let mut cursor = 0;
    while cursor < order.len() {
        let index = order[cursor];
        cursor += 1;
        for child in &children[index] {
            if nodes[*child].attached {
                order.push(*child);
            }
        }
    }
    order
}

fn laterals(
    proxy: &mut Proxy,
    clump_of_guide: &[u32],
    reach_of_guide: &[f32],
    band_widths: BandWidths,
) {
    let guides: Vec<u32> = (0..proxy.positions.len() as u32).step_by(2).collect();
    let reach = band_widths.lateral * LATERAL_REACH;
    let grid = PointGrid::new(
        guides
            .iter()
            .enumerate()
            .map(|(at, guide)| (at as u32, proxy.positions[*guide as usize])),
        reach,
    );
    let mut added: Vec<(u32, u32)> = Vec::new();
    for (at, guide) in guides.iter().enumerate() {
        if proxy.pinned[*guide as usize] {
            continue;
        }
        let position = proxy.positions[*guide as usize];
        let best = grid
            .near(position, reach)
            .filter(|other| {
                clump_of_guide[*other as usize] != clump_of_guide[at]
                    && (reach_of_guide[*other as usize] - reach_of_guide[at]).abs()
                        <= band_widths.band * 1.5
            })
            .map(|other| {
                (
                    proxy.positions[guides[other as usize] as usize].distance(position),
                    other,
                )
            })
            .filter(|(distance, _)| *distance <= reach && *distance > 1e-5)
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        if let Some((_, other)) = best {
            let (a, b) = (*guide, guides[other as usize]);
            added.push((a.min(b), a.max(b)));
        }
    }
    added.sort_unstable();
    added.dedup();
    proxy
        .edges
        .extend(added.into_iter().map(|(a, b)| (a, b, EdgeKind::Lateral)));
}

struct Level {
    clump: u32,
    band: u32,
    reach: f32,
    width: f32,
}

impl Level {
    fn is_adjacent(&self, other: &Level) -> bool {
        if self.clump == other.clump {
            self.band == other.band
        } else {
            (self.reach - other.reach).abs() <= 0.5 * self.width.max(other.width)
        }
    }
}

fn cross_links(proxy: &Proxy, levels: &[Level]) -> Vec<(u32, u32)> {
    let guide = |at: usize| proxy.positions[2 * at];
    let pinned = |at: usize| proxy.pinned[2 * at];
    let clumps = levels
        .iter()
        .map(|level| level.clump + 1)
        .max()
        .unwrap_or(0);
    let mut tails = Union::new(clumps as usize);
    let roots: Vec<usize> = (0..levels.len()).filter(|at| pinned(*at)).collect();
    let root_grid = PointGrid::new(roots.iter().map(|at| (*at as u32, guide(*at))), ROOT_JOIN);
    for at in &roots {
        for other in root_grid.near(guide(*at), ROOT_JOIN) {
            if guide(*at).distance(guide(other as usize)) <= ROOT_JOIN {
                tails.join(levels[*at].clump, levels[other as usize].clump);
            }
        }
    }
    let (tail, _) = tails.labels();
    let grid = PointGrid::new(
        (0..levels.len()).map(|at| (at as u32, guide(at))),
        WIDEST_TUBE,
    );
    let mut candidates: Vec<(f32, u32, u32)> = Vec::new();
    for (at, level) in levels.iter().enumerate() {
        let mut near: Vec<(f32, u32)> = grid
            .near(guide(at), WIDEST_TUBE)
            .filter(|other| {
                let other_level = &levels[*other as usize];
                *other as usize != at
                    && tail[other_level.clump as usize] == tail[level.clump as usize]
                    && !pinned(at)
                    && !pinned(*other as usize)
                    && level.is_adjacent(other_level)
            })
            .map(|other| (guide(at).distance(guide(other as usize)), other))
            .filter(|(distance, _)| *distance <= WIDEST_TUBE && *distance >= MIN_CROSS_LINK_LENGTH)
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut taken: Vec<(u32, usize)> = Vec::new();
        for (distance, other) in near {
            let clump = levels[other as usize].clump;
            let quota = if clump == level.clump {
                CLUMP_CROSS_LINKS
            } else {
                INTER_CLUMP_CROSS_LINKS
            };
            let slot = match taken
                .iter()
                .position(|(taken_clump, _)| *taken_clump == clump)
            {
                Some(slot) => slot,
                None => {
                    taken.push((clump, 0));
                    taken.len() - 1
                }
            };
            if taken[slot].1 < quota {
                taken[slot].1 += 1;
                let (a, b) = (2 * at as u32, 2 * other);
                candidates.push((distance, a.min(b), a.max(b)));
            }
        }
    }
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then((a.1, a.2).cmp(&(b.1, b.2))));
    candidates.dedup_by(|later, earlier| (later.1, later.2) == (earlier.1, earlier.2));
    let mut degree = vec![0usize; proxy.len()];
    let mut cross_links = Vec::new();
    for (_, a, b) in candidates {
        if degree[a as usize] < MAX_CROSS_LINK_DEGREE && degree[b as usize] < MAX_CROSS_LINK_DEGREE
        {
            degree[a as usize] += 1;
            degree[b as usize] += 1;
            cross_links.push((a, b));
        }
    }
    cross_links.sort_unstable();
    cross_links
}
