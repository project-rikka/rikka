use std::collections::BTreeSet;

use rikka_model_core::NodeId;
use rikka_physics_tailor::{
    Anchor, BindTarget, ClothAsset, Curve, Influence, Piece, PieceKind, StrandBinding, Style,
    Underlay,
};

use crate::error::{ClothAssetError, Error, PieceError, Result};

pub(crate) fn asset(asset: &ClothAsset) -> Result<(), ClothAssetError> {
    if asset.pieces.is_empty() {
        return Err(ClothAssetError::NoPieces);
    }
    if !asset
        .rest_worlds
        .iter()
        .all(|world| world.is_finite() && world.determinant() != 0.0)
    {
        return Err(ClothAssetError::RestWorlds);
    }
    let nodes = asset.rest_worlds.len();
    let node = |id: NodeId| id.index() < nodes;
    for (at, vertex) in asset.body.vertices.iter().enumerate() {
        if !vertex.rest.is_finite() || !weights(&vertex.joints, &vertex.weights, node) {
            return Err(ClothAssetError::BodyVertex(at));
        }
    }
    if asset
        .body
        .colliders
        .iter()
        .any(|collider| !node(collider.node))
    {
        return Err(ClothAssetError::ColliderNode);
    }
    let body_vertices = asset.body.vertices.len();
    if let Some(at) = asset.body.triangles.iter().position(|corners| {
        corners
            .iter()
            .any(|&corner| corner as usize >= body_vertices)
    }) {
        return Err(ClothAssetError::BodyTriangle(at));
    }
    let mut drawn = BTreeSet::new();
    for (index, piece) in asset.pieces.iter().enumerate() {
        self::piece(piece, asset, &node).map_err(|error| ClothAssetError::Piece {
            index,
            name: piece.name.clone(),
            error,
        })?;
        for bind in &piece.bindings {
            if !drawn.insert((bind.node, bind.vertex)) {
                return Err(ClothAssetError::DuplicateBind {
                    node: bind.node,
                    vertex: bind.vertex,
                });
            }
        }
    }
    Ok(())
}

fn piece(
    piece: &Piece,
    asset: &ClothAsset,
    node: &impl Fn(NodeId) -> bool,
) -> Result<(), PieceError> {
    let count = piece.particles.len();
    if count == 0 {
        return Err(PieceError::Empty);
    }
    let within = |index: u32| (index as usize) < count;
    for (at, particle) in piece.particles.iter().enumerate() {
        let finite = particle.rest.is_finite()
            && particle.depth.is_finite()
            && particle.radius.is_finite()
            && particle.relief_height.is_finite()
            && particle.relief_depth.is_finite()
            && !particle.reach.is_nan();
        if !finite || !(particle.mass > 0.0 && particle.mass.is_finite()) {
            return Err(PieceError::Particle(at));
        }
        match particle.anchor {
            Anchor::Skinned { joints, weights: w } => {
                if !weights(&joints, &w, node) {
                    return Err(PieceError::SkinWeights(at));
                }
            }
            Anchor::Surface {
                triangle,
                uv,
                offset,
            } => {
                if triangle as usize >= asset.body.triangles.len()
                    || !uv.is_finite()
                    || !offset.is_finite()
                {
                    return Err(PieceError::SurfaceAnchor(at));
                }
            }
        }
        if let Some(parent) = particle.parent {
            if !within(parent) || parent as usize == at {
                return Err(PieceError::Parent { index: at, parent });
            }
            if piece.particles[parent as usize]
                .rest
                .distance(particle.rest)
                <= 1.0e-9
            {
                return Err(PieceError::CoincidentParent(at));
            }
        }
    }
    for (at, edge) in piece.edges.iter().enumerate() {
        let [i, j] = edge.ends;
        if !within(i) || !within(j) || i == j || !(edge.rest > 0.0 && edge.rest.is_finite()) {
            return Err(PieceError::Edge(at));
        }
    }
    for (at, bend) in piece.bends.iter().enumerate() {
        let ids = [bend.hinge[0], bend.hinge[1], bend.wings[0], bend.wings[1]];
        let distinct = ids.iter().collect::<BTreeSet<_>>().len() == 4;
        if !ids.iter().all(|&id| within(id)) || !distinct || !bend.rest_angle.is_finite() {
            return Err(PieceError::Bend(at));
        }
    }
    for (at, tether) in piece.tethers.iter().enumerate() {
        if !within(tether.particle)
            || !within(tether.anchor)
            || !(tether.length >= 0.0 && tether.length.is_finite())
        {
            return Err(PieceError::Tether(at));
        }
    }
    for (at, corners) in piece.triangles.iter().enumerate() {
        if !corners.iter().all(|&corner| within(corner)) {
            return Err(PieceError::Triangle(at));
        }
    }
    for (at, strand_node) in piece.strand_nodes.iter().enumerate() {
        if ![
            strand_node.origin,
            strand_node.side,
            strand_node.along[0],
            strand_node.along[1],
        ]
        .iter()
        .all(|&id| within(id))
        {
            return Err(PieceError::StrandNode(at));
        }
    }
    for (at, bind) in piece.bindings.iter().enumerate() {
        let total = match &bind.target {
            BindTarget::Triangles(influences) => triangle_weight(piece, influences),
            BindTarget::Strand(strand) => strand_node_weight(piece, strand),
        }
        .ok_or(PieceError::BindTarget(at))?;
        if total <= 0.0 || total.is_nan() || !node(bind.node) || !bind.blend.is_finite() {
            return Err(PieceError::BindWeight(at));
        }
        if let Some(under) = bind.underlay
            && !lies_over(asset, piece, under)
        {
            return Err(PieceError::BindUnderlay(at));
        }
    }
    acyclic(piece)
}

fn lies_over(asset: &ClothAsset, piece: &Piece, under: Underlay) -> bool {
    asset.pieces.get(under.piece as usize).is_some_and(|inner| {
        inner.kind == PieceKind::Sheet
            && inner.layer < piece.layer
            && (under.triangle as usize) < inner.triangles.len()
    })
}

fn triangle_weight(piece: &Piece, influences: &[Influence]) -> Option<f32> {
    let mut total = 0.0;
    for influence in influences
        .iter()
        .filter(|influence| influence.weight != 0.0)
    {
        let finite = influence.uv.is_finite()
            && influence.offset.is_finite()
            && influence.normal.is_finite()
            && influence.tangent.is_finite()
            && influence.weight.is_finite();
        if !finite || influence.triangle as usize >= piece.triangles.len() {
            return None;
        }
        total += influence.weight;
    }
    Some(total)
}

fn strand_node_weight(piece: &Piece, strand: &StrandBinding) -> Option<f32> {
    let finite =
        strand.offset.is_finite() && strand.normal.is_finite() && strand.tangent.is_finite();
    let mut total = 0.0;
    for entry in strand.weights.iter().filter(|entry| entry.weight != 0.0) {
        if !finite
            || !(entry.weight > 0.0 && entry.weight.is_finite())
            || entry.strand_node as usize >= piece.strand_nodes.len()
        {
            return None;
        }
        total += entry.weight;
    }
    Some(total)
}

fn acyclic(piece: &Piece) -> Result<(), PieceError> {
    let count = piece.particles.len();
    for start in 0..count {
        let mut at = start;
        let mut hops = 0;
        while let Some(parent) = piece.particles[at].parent {
            at = parent as usize;
            hops += 1;
            if hops > count {
                return Err(PieceError::ParentCycle(start));
            }
        }
    }
    Ok(())
}

fn weights(joints: &[NodeId; 4], weights: &[f32; 4], node: impl Fn(NodeId) -> bool) -> bool {
    let valid = joints.iter().zip(weights).all(|(&joint, &weight)| {
        weight.is_finite() && weight >= 0.0 && (weight == 0.0 || node(joint))
    });
    valid && weights.iter().sum::<f32>() > 0.0
}

pub(crate) fn style(style: &Style) -> Result<()> {
    let scalars = [
        style.gravity,
        style.gravity_falloff,
        style.settling,
        style.drag,
        style.lift,
        style.stretch,
        style.compression,
        style.bend,
        style.buckling,
        style.limit_stiffness,
        style.tether,
        style.extent,
        style.inertia.carried,
        style.inertia.carried_turn,
        style.inertia.depth_carried,
        style.inertia.centrifugal,
        style.inertia.speed_limit,
        style.inertia.turn_limit,
        style.inertia.smoothing,
        style.max_speed,
        style.attenuation,
        style.friction,
        style.thickness,
        style.flutter.amplitude,
        style.flutter.frequency,
        style.flutter.wavelength,
    ];
    let curves = [
        &style.damping,
        &style.restore,
        &style.limit,
        &style.pull,
        &style.max_distance,
    ];
    let finite = scalars.iter().all(|value| !value.is_nan())
        && curves.iter().all(|curve| finite_curve(curve))
        && style.backstop.is_none_or(|distance| !distance.is_nan());
    if finite {
        Ok(())
    } else {
        Err(Error::InvalidStyle)
    }
}

fn finite_curve(curve: &Curve) -> bool {
    curve.points().iter().all(|[_, value]| !value.is_nan())
}
