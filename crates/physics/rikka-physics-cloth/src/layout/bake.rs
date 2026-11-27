use std::f32::consts::PI;

use glam::Vec3;
use rikka_physics_tailor::{
    Anchor, BindTarget, ClothAsset, EdgeKind, Frame, Influence, Piece, PieceKind, StrandBinding,
    Style, particle_normals,
};

use crate::layout::records::{
    AnchorRecord, BindRecord, EdgeRecord, HingeRecord, InfluenceRecord, NONE, RELIEF_LEVELS,
    RELIEF_STEP, STRAND, SURFACE, StrandNodeRecord, StrandRecord, TetherRecord, TraitRecord,
    Writer, octahedral,
};
use crate::layout::topology::{Topology, ancestor_chain};

const STRETCH_LIMIT: f32 = 0.1;
const BUCKLE_ANGLE: f32 = 0.35;
const SHEAR_SHARE: f32 = 0.5;
const LATERAL_SHARE: f32 = 0.25;
const HARDEST_COMPLIANCE: f32 = -9.0;
const COMPLIANCE_DECADES: f32 = 7.0;
const COMPRESSION_DECADES: f32 = 8.5;
const HARDEST_BEND_COMPLIANCE: f32 = -3.0;
const BEND_COMPLIANCE_DECADES: f32 = 5.0;
const MIN_BEND_ASPECT: f32 = 0.05;
const MAX_BEND_ASPECT: f32 = 2.0;
const DENSITY_REF: f32 = 0.25;
const MIN_DENSITY_SCALE: f32 = 0.01;
const MAX_DENSITY_SCALE: f32 = 4.0;
const OFF: f32 = -1.0;
const NEAREST_GAP: f32 = 0.001;
const HELD_CLEARANCE: f32 = 0.02;
const ROOTED: f32 = 3.0;
const MAX_STRAND_ROOTING: f32 = 0.025;
const TETHER_SLACK: f32 = 1.0e-3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Sections {
    pub(crate) rest: u64,
    pub(crate) anchors: u64,
    pub(crate) traits: u64,
    pub(crate) edges: u64,
    pub(crate) bends: u64,
    pub(crate) shapes: u64,
    pub(crate) tether_starts: u64,
    pub(crate) tethers: u64,
    pub(crate) plan: u64,
    pub(crate) triangles: u64,
    pub(crate) incident_starts: u64,
    pub(crate) incident: u64,
    pub(crate) gather_starts: u64,
    pub(crate) wrap_dests: u64,
    pub(crate) binds: u64,
    pub(crate) strand_nodes: u64,
    pub(crate) absorb_starts: u64,
    pub(crate) absorb: u64,
    pub(crate) body_rest: u64,
    pub(crate) body_joints: u64,
    pub(crate) body_weights: u64,
    pub(crate) body_triangles: u64,
}

pub(crate) fn bake(
    topology: &Topology,
    asset: &ClothAsset,
    styles: &[Style],
) -> (Vec<u8>, Sections) {
    let mut writer = Writer::default();
    let rest: Vec<[f32; 4]> = topology
        .order
        .iter()
        .map(|&(piece, particle)| {
            let piece = &asset.pieces[piece];
            let described = &piece.particles[particle];
            let relief = match piece.kind {
                PieceKind::Sheet => packed_relief(described.relief_height, described.relief_depth),
                PieceKind::Strands => 0.0,
            };
            described.rest.extend(relief).to_array()
        })
        .collect();
    let body = &asset.body;
    let skins: Vec<([u32; 4], [f32; 4])> = body
        .vertices
        .iter()
        .map(|vertex| skinning(topology, &vertex.joints, &vertex.weights))
        .collect();
    let sections = Sections {
        rest: writer.section(&rest),
        anchors: writer.section(&anchors(topology, asset)),
        traits: writer.section(&traits(topology, asset, styles)),
        edges: writer.section(&edges(topology, asset, styles)),
        bends: writer.section(&bends(topology, asset, styles)),
        shapes: writer.section(&shapes(topology, asset, styles)),
        tether_starts: writer.section(&topology.tether_starts),
        tethers: writer.section(&tethers(topology, asset, styles)),
        plan: writer.section(&topology.plan),
        triangles: writer.section(&topology.triangles),
        incident_starts: writer.section(&topology.incident_starts),
        incident: writer.section(&topology.incident),
        gather_starts: writer.section(&topology.gather_starts),
        wrap_dests: writer.section(&wrap_dests(topology)),
        binds: writer.section(&binds(topology, asset)),
        strand_nodes: writer.section(&strand_nodes(topology, asset)),
        absorb_starts: writer.section(&topology.absorb_starts),
        absorb: writer.section(&topology.absorb),
        body_rest: writer.section(
            &body
                .vertices
                .iter()
                .map(|vertex| vertex.rest.extend(0.0).to_array())
                .collect::<Vec<_>>(),
        ),
        body_joints: writer.section(&skins.iter().map(|skin| skin.0).collect::<Vec<_>>()),
        body_weights: writer.section(&skins.iter().map(|skin| skin.1).collect::<Vec<_>>()),
        body_triangles: writer.section(
            &body
                .triangles
                .iter()
                .map(|corners| [corners[0], corners[1], corners[2], 0])
                .collect::<Vec<_>>(),
        ),
    };
    (writer.finish(), sections)
}

fn packed_relief(above: f32, below: f32) -> f32 {
    let level =
        |relief: f32| ((relief.max(0.0) / RELIEF_STEP).round() as u32).min(RELIEF_LEVELS - 1);
    (level(above) * RELIEF_LEVELS + level(below)) as f32
}

fn compliance_curve(stiffness: f32, hardest: f32, decades: f32) -> f32 {
    if stiffness > 0.0 {
        10.0f32.powf(hardest + decades * (1.0 - stiffness.min(1.0)))
    } else {
        f32::INFINITY
    }
}

pub(crate) fn compliance(stiffness: f32) -> f32 {
    compliance_curve(stiffness, HARDEST_COMPLIANCE, COMPLIANCE_DECADES)
}

fn compression_compliance(stiffness: f32) -> f32 {
    compliance_curve(stiffness, HARDEST_COMPLIANCE, COMPRESSION_DECADES)
}

fn bend_compliance(stiffness: f32) -> f32 {
    compliance_curve(stiffness, HARDEST_BEND_COMPLIANCE, BEND_COMPLIANCE_DECADES)
}

fn skinning(
    topology: &Topology,
    joints: &[rikka_model_core::NodeId; 4],
    weights: &[f32; 4],
) -> ([u32; 4], [f32; 4]) {
    let total: f32 = weights.iter().sum();
    let mut slots = [0u32; 4];
    let mut shares = [0.0f32; 4];
    for at in 0..4 {
        if weights[at] > 0.0 {
            slots[at] = topology.slot_of[&joints[at]];
            shares[at] = weights[at] / total;
        }
    }
    (slots, shares)
}

fn anchors(topology: &Topology, asset: &ClothAsset) -> Vec<AnchorRecord> {
    topology
        .order
        .iter()
        .enumerate()
        .map(
            |(index, &(piece, particle))| match asset.pieces[piece].particles[particle].anchor {
                Anchor::Skinned { joints, weights } => {
                    let (ids, weights) = skinning(topology, &joints, &weights);
                    AnchorRecord {
                        ids,
                        weights,
                        extra: topology.rest_normals[index].extend(0.0).to_array(),
                    }
                }
                Anchor::Surface {
                    triangle,
                    uv,
                    offset,
                } => AnchorRecord {
                    ids: [triangle, 0, 0, 0],
                    weights: [uv.x, uv.y, 0.0, 0.0],
                    extra: offset.extend(SURFACE).to_array(),
                },
            },
        )
        .collect()
}

fn traits(topology: &Topology, asset: &ClothAsset, styles: &[Style]) -> Vec<TraitRecord> {
    topology
        .order
        .iter()
        .enumerate()
        .map(|(index, &(piece, particle))| {
            let described_piece = &asset.pieces[piece];
            let described = &described_piece.particles[particle];
            let style = &styles[piece];
            let depth = described.depth.clamp(0.0, 1.0);
            let reach = style.reach_at(described.reach, described.spread, depth);
            let carry = style.inertia.depth_carried * (1.0 - depth * depth);
            let radius = described.radius.max(NEAREST_GAP);
            let self_thickness = match described_piece.kind {
                PieceKind::Strands => radius.min(style.thickness.max(NEAREST_GAP)),
                PieceKind::Sheet => style.thickness.max(NEAREST_GAP),
            };
            let rooting = match described_piece.kind {
                PieceKind::Strands => (ROOTED * radius).min(MAX_STRAND_ROOTING),
                PieceKind::Sheet => ROOTED * self_thickness,
            };
            let inside = described.clearance < -radius
                || (described.clearance < NEAREST_GAP && topology.root_distance[index] < rooting);
            let unrooted =
                (topology.root_distance[index] / rooting.max(NEAREST_GAP)).clamp(0.0, 1.0);
            let body_radius = if inside {
                OFF
            } else {
                (described.clearance * unrooted).clamp(NEAREST_GAP, radius)
            };
            let held_clearance = if inside {
                OFF
            } else {
                described.clearance.min(HELD_CLEARANCE).max(body_radius)
            };
            TraitRecord {
                a: [
                    topology.inverse_mass[index],
                    style.damping.sample(depth).clamp(0.0, 1.0),
                    style.pull.sample(depth).clamp(0.0, 1.0),
                    if reach.is_finite() && reach >= 0.0 {
                        reach
                    } else {
                        OFF
                    },
                ],
                b: [
                    topology.aero_areas[index],
                    style
                        .backstop_at(described_piece.kind, depth)
                        .unwrap_or(OFF),
                    self_thickness,
                    body_radius,
                ],
                c: [
                    carry.clamp(0.0, 1.0),
                    topology.root_distance[index],
                    held_clearance,
                    1.0 / described.mass,
                ],
                d: [
                    piece as u32,
                    topology.components[index],
                    topology.shares[index].wraps.to_bits(),
                    depth.to_bits(),
                ],
            }
        })
        .collect()
}

fn wrap_dests(topology: &Topology) -> Vec<[u32; 4]> {
    (0..topology.triangles.len() as u32)
        .map(|item| {
            let at = topology.bases.wraps + 3 * item;
            [
                topology.dest(at),
                topology.dest(at + 1),
                topology.dest(at + 2),
                0,
            ]
        })
        .collect()
}

fn edges(topology: &Topology, asset: &ClothAsset, styles: &[Style]) -> Vec<EdgeRecord> {
    topology
        .edges
        .iter()
        .map(|&(piece, index)| {
            let edge = asset.pieces[piece].edges[index];
            let style = &styles[piece];
            let share = match edge.kind {
                EdgeKind::Structural => 1.0,
                EdgeKind::Shear => SHEAR_SHARE,
                EdgeKind::Lateral => LATERAL_SHARE,
            };
            let [i, j] = edge.ends.map(|end| topology.gpu[piece][end as usize]);
            let [wi, wj] = [i, j].map(|id| topology.inverse_mass[id as usize]);
            let particles = &asset.pieces[piece].particles;
            let mass = 0.5
                * (particles[edge.ends[0] as usize].mass + particles[edge.ends[1] as usize].mass);
            let scale = density_scale(edge.rest * edge.rest, mass);
            let longest = if style.stretch > 0.0 {
                edge.rest * (1.0 + STRETCH_LIMIT)
            } else {
                f32::MAX
            };
            EdgeRecord {
                ends: [i, j, edge.rest.to_bits(), longest.to_bits()],
                params: [
                    compliance(style.stretch * share),
                    compression_compliance(style.compression * share) * scale,
                    wi,
                    wj,
                ],
            }
        })
        .collect()
}

fn density_scale(area: f32, mass: f32) -> f32 {
    (area / mass.max(f32::MIN_POSITIVE) * DENSITY_REF).clamp(MIN_DENSITY_SCALE, MAX_DENSITY_SCALE)
}

fn bends(topology: &Topology, asset: &ClothAsset, styles: &[Style]) -> Vec<HingeRecord> {
    topology
        .bends
        .iter()
        .enumerate()
        .map(|(item, &(piece, index))| {
            let at = topology.bases.bends + 4 * item as u32;
            let bend = asset.pieces[piece].bends[index];
            let style = &styles[piece];
            let corners = [bend.hinge[0], bend.hinge[1], bend.wings[0], bend.wings[1]];
            let ids = corners.map(|particle| topology.gpu[piece][particle as usize]);
            let buckling = style.buckling.clamp(0.0, 1.0);
            let particles = &asset.pieces[piece].particles;
            let [p0, p1, p2, p3] = corners.map(|particle| particles[particle as usize].rest);
            let edge = p1 - p0;
            let hinge = edge.length_squared();
            let areas = 0.5 * (edge.cross(p2 - p0).length() + edge.cross(p3 - p0).length());
            let aspect = (areas / (3.0 * hinge.max(f32::MIN_POSITIVE)))
                .clamp(MIN_BEND_ASPECT, MAX_BEND_ASPECT);
            let mass = 0.25
                * corners
                    .into_iter()
                    .map(|particle| particles[particle as usize].mass)
                    .sum::<f32>();
            let bend_scale = aspect * density_scale(areas, mass);
            let weights = ids.map(|id| topology.inverse_mass[id as usize]);
            HingeRecord {
                ids,
                params: [
                    bend.rest_angle,
                    bend_compliance(style.bend) * bend_scale,
                    bend_compliance(style.bend * (1.0 - buckling)) * bend_scale,
                    if buckling > 0.0 {
                        BUCKLE_ANGLE
                    } else {
                        f32::MAX
                    },
                ],
                weights,
                applied: weights.map(|weight| weight * topology.bend_share(&ids)),
                dests: std::array::from_fn(|k| topology.dest(at + k as u32)),
            }
        })
        .collect()
}

fn shapes(topology: &Topology, asset: &ClothAsset, styles: &[Style]) -> Vec<HingeRecord> {
    topology
        .shapes
        .iter()
        .enumerate()
        .map(|(item, &(piece, child))| {
            let at = topology.bases.shapes + 3 * item as u32;
            let described_piece = &asset.pieces[piece];
            let chain = ancestor_chain(described_piece, child as u32);
            let rest = |at: usize| described_piece.particles[chain[at] as usize].rest;
            let rest_angle = match chain.len() {
                3 => (rest(0) - rest(1)).angle_between(rest(1) - rest(2)),
                _ => 0.0,
            };
            let described = &described_piece.particles[child];
            let style = &styles[piece];
            let depth = described.depth.clamp(0.0, 1.0);
            let limit = style.limit.sample(depth) * described.spread.max(0.0);
            let mut ids = [NONE; 4];
            let mut weights = [0.0; 4];
            for (at, &particle) in chain.iter().enumerate() {
                ids[at] = topology.gpu[piece][particle as usize];
                weights[at] = topology.inverse_mass[ids[at] as usize];
            }
            let share = topology.shape_share(&ids);
            let applied = weights.map(|weight| weight * share);
            HingeRecord {
                ids,
                params: [
                    rest_angle,
                    if limit > 0.0 && limit < PI {
                        limit
                    } else {
                        OFF
                    },
                    style.limit_stiffness.clamp(0.0, 1.0),
                    style.restore.sample(depth).clamp(0.0, 1.0),
                ],
                weights,
                applied,
                dests: [
                    topology.dest(at),
                    topology.dest(at + 1),
                    topology.dest(at + 2),
                    NONE,
                ],
            }
        })
        .collect()
}

fn tethers(topology: &Topology, asset: &ClothAsset, styles: &[Style]) -> Vec<TetherRecord> {
    topology
        .tethers
        .iter()
        .map(|&(piece, index)| {
            let described_piece = &asset.pieces[piece];
            let tether = described_piece.tethers[index];
            let scale = styles[piece].tether;
            TetherRecord {
                anchor: topology.gpu[piece][tether.anchor as usize],
                length: if scale > 0.0 {
                    tether.length * scale * (1.0 + TETHER_SLACK)
                } else {
                    f32::MAX
                },
                shortest: described_piece.particles[tether.particle as usize]
                    .rest
                    .distance(described_piece.particles[tether.anchor as usize].rest)
                    * styles[piece].extent,
                padding: 0,
            }
        })
        .collect()
}

fn binds(topology: &Topology, asset: &ClothAsset) -> Vec<BindRecord> {
    let strand_node_bases = strand_node_bases(asset);
    let resting: Vec<Resting<'_>> = asset.pieces.iter().map(Resting::new).collect();
    topology
        .binds
        .iter()
        .map(|&(piece, index)| {
            let bind = asset.pieces[piece].bindings[index];
            let slot = topology.nodes.binary_search(&bind.node).unwrap_or(0) as u32;
            let (kind, target) = match bind.target {
                BindTarget::Triangles(influences) => (
                    0,
                    on_triangles(topology, piece, &influences, &resting[piece]),
                ),
                BindTarget::Strand(strand) => {
                    (STRAND, on_strand(strand_node_bases[piece], &strand))
                }
            };
            BindRecord {
                header: [
                    slot | kind,
                    bind.vertex,
                    bind.blend.clamp(0.0, 1.0).to_bits(),
                    bind.gap.to_bits(),
                ],
                target,
            }
        })
        .collect()
}

struct Resting<'a> {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    piece: &'a Piece,
}

impl<'a> Resting<'a> {
    fn new(piece: &'a Piece) -> Resting<'a> {
        let positions = piece.rest_positions();
        Resting {
            normals: particle_normals(&positions, &piece.triangles),
            positions,
            piece,
        }
    }

    fn rise(&self, influence: &Influence) -> f32 {
        let corners = self.piece.triangles[influence.triangle as usize];
        Frame::from_triangle(
            corners.map(|corner| self.positions[corner as usize]),
            corners.map(|corner| self.normals[corner as usize]),
            influence.uv,
        )
        .normal
        .y
    }
}

fn on_triangles(
    topology: &Topology,
    piece: usize,
    influences: &[Influence],
    resting: &Resting<'_>,
) -> [[u32; 4]; 10] {
    let total: f32 = influences.iter().map(|influence| influence.weight).sum();
    let mut records = [InfluenceRecord::default(); 3];
    for (slot, influence) in records.iter_mut().zip(influences) {
        if influence.weight == 0.0 {
            continue;
        }
        let [nx, ny] = octahedral(influence.normal);
        let [tx, ty] = octahedral(influence.tangent);
        *slot = InfluenceRecord {
            triangle: topology.triangle_base[piece] + influence.triangle,
            place: [influence.uv.x, influence.uv.y, influence.weight / total],
            offset: influence.offset.extend(resting.rise(influence)).to_array(),
            directions: [nx, ny, tx, ty],
        };
    }
    let mut target = [[0u32; 4]; 10];
    let words: &[[u32; 4]] = bytemuck::cast_slice(&records);
    target[..words.len()].copy_from_slice(words);
    target
}

fn on_strand(base: u32, strand: &StrandBinding) -> [[u32; 4]; 10] {
    let total: f32 = strand.weights.iter().map(|entry| entry.weight).sum();
    let mut record = StrandRecord::default();
    let weights = strand.weights.iter().filter(|entry| entry.weight > 0.0);
    for ((strand_node, weight), entry) in record
        .strand_nodes
        .iter_mut()
        .zip(record.weights.iter_mut())
        .zip(weights)
    {
        *strand_node = base + entry.strand_node;
        *weight = entry.weight / total;
    }
    let [nx, ny] = octahedral(strand.normal);
    let [tx, ty] = octahedral(strand.tangent);
    record.offset = strand.offset.extend(0.0).to_array();
    record.directions = [nx, ny, tx, ty];
    bytemuck::cast(record)
}

fn strand_node_bases(asset: &ClothAsset) -> Vec<u32> {
    asset
        .pieces
        .iter()
        .scan(0u32, |base, piece| {
            let start = *base;
            *base += piece.strand_nodes.len() as u32;
            Some(start)
        })
        .collect()
}

fn strand_nodes(topology: &Topology, asset: &ClothAsset) -> Vec<StrandNodeRecord> {
    asset
        .pieces
        .iter()
        .enumerate()
        .flat_map(|(piece, described_piece)| {
            let rest = described_piece.rest_positions();
            let gpu = &topology.gpu[piece];
            described_piece.strand_nodes.iter().map(move |strand_node| {
                let frame = strand_node.frame(&rest);
                let [from, to] = strand_node.along.map(|end| gpu[end as usize]);
                StrandNodeRecord {
                    ids: [
                        gpu[strand_node.origin as usize],
                        gpu[strand_node.side as usize],
                        from,
                        to,
                    ],
                    tangent: frame.tangent.extend(0.0).to_array(),
                    bitangent: frame.bitangent.extend(0.0).to_array(),
                }
            })
        })
        .collect()
}
