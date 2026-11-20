mod geometry;
mod layers;
mod remap;

use glam::Vec3;
use rikka_polymesh::{
    BEVEL_WEIGHT_EDGE, BEVEL_WEIGHT_VERT, CREASE_EDGE, DataType, Domain, Polymesh, SHARP_EDGE,
    SHARP_FACE, UV_SEAM,
};

use super::{Scope, target};
use crate::kinds::data_transfer::{
    CornerData, CornerMapping, DataTransfer, EdgeData, EdgeMapping, FaceData, FaceMapping,
    VertexData, VertexMapping,
};
use crate::note::{NoteKind, SkipReason, TransferFailure};
use crate::space::SpaceTransform;
use geometry::slerp_safe;
use layers::{Mix, transfer_bools, transfer_floats, transfer_layers, transfer_vertex_groups};
use remap::{Query, Sources};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    VertexGroups,
    VertexBevel,
    EdgeSharp,
    EdgeSeam,
    EdgeCrease,
    EdgeBevel,
    VertexColor,
    CustomNormal,
    Uv,
    FaceSmooth,
    CornerColor,
}

impl Kind {
    const ORDER: [Kind; 11] = [
        Kind::VertexGroups,
        Kind::VertexBevel,
        Kind::EdgeSharp,
        Kind::EdgeSeam,
        Kind::EdgeCrease,
        Kind::EdgeBevel,
        Kind::VertexColor,
        Kind::CustomNormal,
        Kind::Uv,
        Kind::FaceSmooth,
        Kind::CornerColor,
    ];

    fn domain(self) -> Domain {
        match self {
            Kind::VertexGroups | Kind::VertexBevel | Kind::VertexColor => Domain::Point,
            Kind::EdgeSharp | Kind::EdgeSeam | Kind::EdgeCrease | Kind::EdgeBevel => Domain::Edge,
            Kind::CustomNormal | Kind::Uv | Kind::CornerColor => Domain::Corner,
            Kind::FaceSmooth => Domain::Face,
        }
    }

    fn selected(self, transfer: &DataTransfer) -> bool {
        let vertex =
            |data: VertexData| transfer.use_vert_data && transfer.data_types_verts.contains(data);
        let edge =
            |data: EdgeData| transfer.use_edge_data && transfer.data_types_edges.contains(data);
        let corner =
            |data: CornerData| transfer.use_loop_data && transfer.data_types_loops.contains(data);
        let face =
            |data: FaceData| transfer.use_poly_data && transfer.data_types_polys.contains(data);
        match self {
            Kind::VertexGroups => vertex(VertexData::VERTEX_GROUPS),
            Kind::VertexBevel => vertex(VertexData::BEVEL_WEIGHT),
            Kind::VertexColor => vertex(VertexData::COLOR),
            Kind::EdgeSharp => edge(EdgeData::SHARP),
            Kind::EdgeSeam => edge(EdgeData::SEAM),
            Kind::EdgeCrease => edge(EdgeData::CREASE),
            Kind::EdgeBevel => edge(EdgeData::BEVEL_WEIGHT),
            Kind::CustomNormal => corner(CornerData::CUSTOM_NORMAL),
            Kind::Uv => corner(CornerData::UV),
            Kind::CornerColor => corner(CornerData::COLOR),
            Kind::FaceSmooth => face(FaceData::SMOOTH),
        }
    }
}

#[derive(Default)]
struct Maps {
    vertices: Option<Vec<Sources>>,
    edges: Option<Vec<Sources>>,
    corners: Option<Vec<Sources>>,
    faces: Option<Vec<Sources>>,
}

pub(crate) fn evaluate(
    transfer: &DataTransfer,
    mesh: &mut Polymesh,
    scope: &mut Scope<'_>,
) -> Result<(), SkipReason> {
    let ctx = scope.ctx;
    let object = transfer.object.ok_or(SkipReason::MissingObject)?;
    let source = target(ctx, object)?;
    let transform = match transfer.use_object_transform {
        true => Some(SpaceTransform::new(
            ctx.object_matrix(),
            ctx.matrix(object).ok_or(SkipReason::MissingObject)?,
        )),
        false => None,
    };
    let query = Query {
        source: &source,
        transform,
        max_distance: match transfer.use_max_distance {
            true => transfer.max_distance,
            false => f32::MAX,
        },
        ray_radius: transfer.ray_radius,
    };
    let group = (!transfer.vertex_group.is_empty() && mesh.vertex_groups.has_weights())
        .then(|| mesh.vertex_groups.index(&transfer.vertex_group))
        .flatten();
    let mut maps = Maps::default();
    let mut first_corner_kind = None;
    for kind in Kind::ORDER {
        if !kind.selected(transfer) {
            continue;
        }
        let domain = kind.domain();
        if domain == Domain::Corner && first_corner_kind.is_none() {
            first_corner_kind = Some(kind);
        }
        let map = match map_for(
            &mut maps,
            transfer,
            &query,
            mesh,
            domain,
            first_corner_kind == Some(Kind::Uv),
        ) {
            Ok(map) => map,
            Err(failure) => {
                scope.note(NoteKind::TransferFailed(failure));
                continue;
            }
        };
        let weights =
            group.map(|group| mask_weights(mesh, group, domain, transfer.invert_vertex_group));
        let mix = Mix {
            mode: transfer.mix_mode,
            factor: transfer.mix_factor,
            weights: weights.as_deref(),
        };
        let source_mesh = source.mesh();
        match kind {
            Kind::VertexGroups => transfer_vertex_groups(
                source_mesh,
                mesh,
                map,
                transfer.layers_vgroup,
                &mix,
                ctx.deforming_bones(object),
            ),
            Kind::VertexBevel => {
                transfer_floats(source_mesh, mesh, map, BEVEL_WEIGHT_VERT, Domain::Point)
            }
            Kind::EdgeBevel => {
                transfer_floats(source_mesh, mesh, map, BEVEL_WEIGHT_EDGE, Domain::Edge)
            }
            Kind::EdgeCrease => transfer_floats(source_mesh, mesh, map, CREASE_EDGE, Domain::Edge),
            Kind::EdgeSharp => transfer_bools(source_mesh, mesh, map, SHARP_EDGE, Domain::Edge),
            Kind::EdgeSeam => transfer_bools(source_mesh, mesh, map, UV_SEAM, Domain::Edge),
            Kind::FaceSmooth => transfer_bools(source_mesh, mesh, map, SHARP_FACE, Domain::Face),
            Kind::VertexColor => transfer_layers(
                source_mesh,
                mesh,
                map,
                DataType::Color,
                Domain::Point,
                transfer.layers_vcol_vert,
                &mix,
            ),
            Kind::CornerColor => transfer_layers(
                source_mesh,
                mesh,
                map,
                DataType::Color,
                Domain::Corner,
                transfer.layers_vcol_loop,
                &mix,
            ),
            Kind::Uv => transfer_layers(
                source_mesh,
                mesh,
                map,
                DataType::Vec2,
                Domain::Corner,
                transfer.layers_uv,
                &mix,
            ),
            Kind::CustomNormal => transfer_normals(&source, mesh, map, transform, &mix),
        }
    }
    Ok(())
}

fn map_for<'m>(
    maps: &'m mut Maps,
    transfer: &DataTransfer,
    query: &Query<'_>,
    mesh: &Polymesh,
    domain: Domain,
    islands: bool,
) -> Result<&'m [Sources], TransferFailure> {
    let cached = match domain {
        Domain::Point => &mut maps.vertices,
        Domain::Edge => &mut maps.edges,
        Domain::Corner => &mut maps.corners,
        Domain::Face => &mut maps.faces,
    };
    if cached.is_none() {
        *cached = Some(build_map(transfer, query, mesh, domain, islands)?);
    }
    Ok(cached.get_or_insert_with(Vec::new))
}

fn build_map(
    transfer: &DataTransfer,
    query: &Query<'_>,
    mesh: &Polymesh,
    domain: Domain,
    islands: bool,
) -> Result<Vec<Sources>, TransferFailure> {
    let source = query.source.mesh();
    Ok(match domain {
        Domain::Point => {
            let mode = transfer.vert_mapping;
            if mode == VertexMapping::Topology && mesh.point_count() != source.point_count() {
                return Err(TransferFailure::TopologyMismatch(Domain::Point));
            }
            if matches!(
                mode,
                VertexMapping::EdgeNearest | VertexMapping::EdgeInterpolatedNearest
            ) && source.edge_count() == 0
            {
                return Err(TransferFailure::NoSourceEdges);
            }
            if matches!(
                mode,
                VertexMapping::FaceNearest
                    | VertexMapping::FaceInterpolatedNearest
                    | VertexMapping::FaceInterpolatedNormalProjected
            ) && source.face_count() == 0
            {
                return Err(TransferFailure::NoSourceFaces);
            }
            if mesh.point_count() == 0 || source.point_count() == 0 {
                return Err(TransferFailure::NoElements(Domain::Point));
            }
            remap::vertices(mode, query, mesh)
        }
        Domain::Edge => {
            let mode = transfer.edge_mapping;
            if mode == EdgeMapping::Topology && mesh.edge_count() != source.edge_count() {
                return Err(TransferFailure::TopologyMismatch(Domain::Edge));
            }
            if mode == EdgeMapping::FaceNearest && source.face_count() == 0 {
                return Err(TransferFailure::NoSourceFaces);
            }
            if mesh.edge_count() == 0 || source.edge_count() == 0 {
                return Err(TransferFailure::NoElements(Domain::Edge));
            }
            remap::edges(mode, query, mesh)
        }
        Domain::Corner => {
            let mode = transfer.loop_mapping;
            if mode == CornerMapping::Topology && mesh.corner_count() != source.corner_count() {
                return Err(TransferFailure::TopologyMismatch(Domain::Corner));
            }
            if mesh.corner_count() == 0 || source.corner_count() == 0 {
                return Err(TransferFailure::NoElements(Domain::Corner));
            }
            remap::corners(mode, query, mesh, islands)
        }
        Domain::Face => {
            let mode = transfer.poly_mapping;
            if mode == FaceMapping::Topology && mesh.face_count() != source.face_count() {
                return Err(TransferFailure::TopologyMismatch(Domain::Face));
            }
            if mesh.face_count() == 0 || source.face_count() == 0 {
                return Err(TransferFailure::NoElements(Domain::Face));
            }
            remap::faces(mode, query, mesh)
        }
    })
}

fn mask_weights(mesh: &Polymesh, group: u32, domain: Domain, invert: bool) -> Vec<f32> {
    let weight = |point: u32| {
        let weight = mesh.vertex_groups.weight(point as usize, group);
        match invert {
            true => 1.0 - weight,
            false => weight,
        }
    };
    match domain {
        Domain::Point => (0..mesh.point_count() as u32).map(weight).collect(),
        Domain::Edge => mesh
            .edges
            .iter()
            .map(|&[a, b]| (weight(a) + weight(b)) * 0.5)
            .collect(),
        Domain::Corner => mesh
            .corner_verts
            .iter()
            .map(|&point| weight(point))
            .collect(),
        Domain::Face => (0..mesh.face_count())
            .map(|face| {
                let points = mesh.face_verts(face);
                points.iter().map(|&point| weight(point)).sum::<f32>() / points.len() as f32
            })
            .collect(),
    }
}

fn transfer_normals(
    source: &crate::target::Target,
    mesh: &mut Polymesh,
    map: &[Sources],
    transform: Option<SpaceTransform>,
    mix: &Mix<'_>,
) {
    let source_normals = source.corner_normals();
    let mut normals = mesh.corner_normals();
    for (corner, sources) in map.iter().enumerate() {
        if sources.is_empty() {
            continue;
        }
        let sum = sources.iter().fold(Vec3::ZERO, |sum, &(index, weight)| {
            sum + source_normals[index as usize] * weight
        });
        let mut value = crate::math::normalize(sum);
        if let Some(transform) = transform {
            value = transform.invert_normal(value);
        }
        normals[corner] = mix_normal(mix, corner, normals[corner], value);
    }
    let encoded = mesh.encode_custom_normals(&normals);
    let decoded = mesh.decode_custom_normals(&encoded);
    mesh.custom_normals_mut().copy_from_slice(&decoded);
}

fn mix_normal(mix: &Mix<'_>, element: usize, destination: Vec3, source: Vec3) -> Vec3 {
    use crate::kinds::data_transfer::MixMode;
    let factor = mix.factor_at(element);
    let target = match mix.mode {
        MixMode::ReplaceAboveThreshold | MixMode::ReplaceBelowThreshold => return source,
        MixMode::Mix | MixMode::Add => crate::math::normalize(destination + source),
        MixMode::Subtract => crate::math::normalize(destination - source),
        MixMode::Multiply => crate::math::normalize(destination * source),
        MixMode::Replace => source,
    };
    slerp_safe(destination, target, factor)
}
