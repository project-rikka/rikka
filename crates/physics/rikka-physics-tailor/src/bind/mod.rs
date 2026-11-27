mod patches;

use glam::Vec3;

use crate::asset::{
    BindTarget, Influence, MAX_INFLUENCES, StrandBinding, StrandWeight, VertexBinding,
};
use crate::figure::{Figure, RestVertex};
use crate::frame::Frame;
use crate::geometry::{Hit, PointGrid, Shell, TriangleTree, Union};
use crate::proxy::Proxy;
use patches::Patches;

const REACH_SHARE: f32 = 0.75;
const BLEND_REACH: f32 = 0.03;
const SKIN_GAP: f32 = 0.001;
const LAYER_GAPS: f32 = 0.004;
const TIED: f32 = 1e-4;
const CANDIDATES: usize = 12;
const CENTRE_SHARE: f32 = 0.5;
const UNBOUND_SPACINGS: f32 = 4.0;
const RIDING: f32 = 0.08;

pub(crate) struct Binding<'a> {
    pub(crate) normals: &'a [Vec3],
    pub(crate) depth: &'a [f32],
    pub(crate) deepest: f32,
    pub(crate) spacing: f32,
    pub(crate) body: &'a Shell,
    pub(crate) far: f32,
    pub(crate) triangles: &'a TriangleTree,
}

struct BindCandidate {
    vertex: u32,
    hold: BindTarget,
    depth: f32,
    lift: f32,
    thickness: f32,
    share: f32,
}
pub(crate) struct Bound {
    pub(crate) bind: VertexBinding,
    pub(crate) thickness: f32,
}

pub(crate) fn bind(figure: &Figure<'_>, proxy: &Proxy, binding: &Binding<'_>) -> Vec<Bound> {
    let blend_depth = if binding.deepest > 0.0 {
        (BLEND_REACH / binding.deepest).clamp(0.02, 0.5)
    } else {
        1.0
    };
    let candidates = if proxy.strand_nodes.is_empty() {
        on_triangles(figure, proxy, binding)
    } else {
        on_strands(figure, proxy, binding)
    };
    let floor = candidates
        .iter()
        .map(|candidate| candidate.lift)
        .fold(f32::MAX, f32::min);
    let ceiling = candidates
        .iter()
        .map(|candidate| candidate.lift)
        .fold(f32::MIN, f32::max);
    let layering = if ceiling > floor {
        LAYER_GAPS / (ceiling - floor)
    } else {
        0.0
    };
    candidates
        .into_iter()
        .map(|candidate| {
            let key = figure.key(candidate.vertex);
            Bound {
                bind: VertexBinding {
                    node: key.node,
                    vertex: key.vertex,
                    blend: candidate.share * smoothstep(blend_depth, candidate.depth),
                    gap: binding
                        .body
                        .signed(
                            figure.vertices[candidate.vertex as usize].position,
                            binding.far,
                        )
                        .map_or(binding.far, |signed| signed.distance)
                        .min(SKIN_GAP + (candidate.lift - floor) * layering),
                    target: candidate.hold,
                },
                thickness: candidate.thickness,
            }
        })
        .collect()
}

fn on_strands(figure: &Figure<'_>, proxy: &Proxy, binding: &Binding<'_>) -> Vec<BindCandidate> {
    proxy
        .vertex_map
        .iter()
        .zip(&proxy.node_weights)
        .map(|((vertex, _), node_weights)| {
            let mut strand = StrandBinding::default();
            let mut depth = 0.0;
            for (slot, (strand_node, weight)) in strand.weights.iter_mut().zip(node_weights) {
                *slot = StrandWeight {
                    strand_node: *strand_node,
                    weight: *weight,
                };
                depth += weight
                    * binding.depth[proxy.strand_nodes[*strand_node as usize].origin as usize];
            }
            let rest = &figure.vertices[*vertex as usize];
            let leader = proxy.strand_nodes[node_weights[0].0 as usize].frame(&proxy.positions);
            strand.offset = rest.position - strand.origin(&proxy.strand_nodes, &proxy.positions);
            strand.normal = if rest.normal.length_squared() > 0.5 {
                rest.normal
            } else {
                leader.normal
            };
            strand.tangent = if rest.tangent.length_squared() > 0.5 {
                rest.tangent
            } else {
                leader.tangent
            };
            BindCandidate {
                vertex: *vertex,
                share: 1.0,
                thickness: strand
                    .offset
                    .reject_from_normalized(leader.tangent)
                    .length(),
                hold: BindTarget::Strand(strand),
                depth,
                lift: 0.0,
            }
        })
        .collect()
}

fn on_triangles(figure: &Figure<'_>, proxy: &Proxy, binding: &Binding<'_>) -> Vec<BindCandidate> {
    let everywhere = binding.triangles;
    let patches = Patches::new(figure, proxy);
    let width = (binding.spacing * REACH_SHARE).max(1e-4);
    let welded = welded(figure, proxy);
    let mut candidates = Vec::with_capacity(proxy.vertex_map.len());
    let mut aparts: Vec<(f32, u32)> = Vec::with_capacity(proxy.vertex_map.len());
    for (at, (vertex, _)) in proxy.vertex_map.iter().enumerate() {
        let rest = &figure.vertices[*vertex as usize];
        let point = rest.position;
        let seed = welded[at];
        let mut found: Vec<(f32, Hit)> = everywhere
            .nearest_few_where(seed, CANDIDATES, f32::INFINITY, |triangle| {
                patches.admits(at, triangle)
            })
            .into_iter()
            .map(|hit| {
                let centre = everywhere.corners(hit.triangle).iter().sum::<Vec3>() / 3.0;
                (hit.distance + CENTRE_SHARE * seed.distance(centre), hit)
            })
            .collect();
        found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.triangle.cmp(&b.1.triangle)));
        found.truncate(MAX_INFLUENCES + 1);
        let Some((_, nearest)) = found.first() else {
            continue;
        };
        let distances: Vec<f32> = found.iter().map(|(key, _)| *key).collect();
        let shares = shares(&distances, width);
        let mut influences = [Influence::default(); MAX_INFLUENCES];
        let mut depth = 0.0;
        let mut lift = 0.0;
        let thickness = {
            let ids = proxy.triangles[nearest.triangle as usize];
            Frame::from_triangle(
                ids.map(|corner| proxy.positions[corner as usize]),
                ids.map(|corner| binding.normals[corner as usize]),
                nearest.closest.uv,
            )
            .local_point(point)
            .z
            .abs()
        };
        for (slot, ((_, hit), weight)) in influences.iter_mut().zip(
            found
                .iter()
                .zip(&shares)
                .filter(|(_, weight)| **weight > 0.0),
        ) {
            let ids = proxy.triangles[hit.triangle as usize];
            let uv = hit.closest.uv;
            let frame = Frame::from_triangle(
                ids.map(|corner| proxy.positions[corner as usize]),
                ids.map(|corner| binding.normals[corner as usize]),
                uv,
            );
            let (normal, tangent) = directions(&frame, rest);
            let offset = frame.local_point(point);
            *slot = Influence {
                triangle: hit.triangle,
                uv,
                offset,
                normal,
                tangent,
                weight: *weight,
            };
            let [a, b, c] = ids.map(|corner| binding.depth[corner as usize]);
            depth += weight * (a * (1.0 - uv.x - uv.y) + b * uv.x + c * uv.y);
            lift += weight * offset.z;
        }
        candidates.push(BindCandidate {
            vertex: *vertex,
            hold: BindTarget::Triangles(influences),
            depth,
            lift,
            share: 1.0,
            thickness,
        });
        aparts.push((nearest.distance, patches.component(at)));
    }
    let unbound = (UNBOUND_SPACINGS * binding.spacing).max(RIDING);
    let mut closest = vec![f32::INFINITY; patches.count()];
    for (apart, component) in &aparts {
        let nearest = &mut closest[*component as usize];
        *nearest = nearest.min(*apart);
    }
    for (candidate, (apart, component)) in candidates.iter_mut().zip(&aparts) {
        candidate.share = if closest[*component as usize] > unbound {
            0.0
        } else {
            1.0 - smoothstep(0.5 * unbound, apart - 0.5 * unbound)
        };
    }
    candidates
}

fn welded(figure: &Figure<'_>, proxy: &Proxy) -> Vec<Vec3> {
    let same = figure.same_distance();
    let position = |at: usize| figure.vertices[proxy.vertex_map[at].0 as usize].position;
    let grid = PointGrid::new(
        (0..proxy.vertex_map.len()).map(|at| (at as u32, position(at))),
        same * 2.0,
    );
    let mut union = Union::new(proxy.vertex_map.len());
    for at in 0..proxy.vertex_map.len() {
        for other in grid.near(position(at), same) {
            if other as usize > at && position(other as usize).distance(position(at)) <= same {
                union.join(at as u32, other);
            }
        }
    }
    let (labels, count) = union.labels();
    let mut sums = vec![(Vec3::ZERO, 0u32); count];
    for (at, label) in labels.iter().enumerate() {
        sums[*label as usize].0 += position(at);
        sums[*label as usize].1 += 1;
    }
    labels
        .iter()
        .map(|label| {
            let (sum, count) = sums[*label as usize];
            sum / count as f32
        })
        .collect()
}

fn shares(distances: &[f32], width: f32) -> Vec<f32> {
    let nearest = distances[0];
    let falloff = |distance: f32| (1.0 - (distance - nearest) / width).max(0.0).powi(3);
    let floor = distances
        .get(MAX_INFLUENCES)
        .map_or(0.0, |distance| falloff(*distance));
    let nearest = &distances[..distances.len().min(MAX_INFLUENCES)];
    let mut raw: Vec<f32> = nearest
        .iter()
        .map(|distance| falloff(*distance) - floor)
        .collect();
    if raw.iter().sum::<f32>() <= TIED {
        raw = nearest.iter().map(|distance| falloff(*distance)).collect();
    }
    let total: f32 = raw.iter().sum();
    raw.into_iter().map(|share| share / total).collect()
}

fn directions(frame: &Frame, rest: &RestVertex) -> (Vec3, Vec3) {
    let normal = if rest.normal.length_squared() > 0.5 {
        frame.local_vector(rest.normal)
    } else {
        Vec3::Z
    };
    let tangent = if rest.tangent.length_squared() > 0.5 {
        frame.local_vector(rest.tangent)
    } else {
        Vec3::X
    };
    (normal, tangent)
}

fn smoothstep(edge: f32, value: f32) -> f32 {
    let t = (value / edge).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
