use rikka_mesh_modifiers::{
    CornerData, CornerMapping, DataTransfer, EdgeData, EdgeMapping, FaceData, FaceMapping,
    LayerSelection, LayersDestination, LayersSource, MixMode, VertexData, VertexMapping,
};

use super::{References, Targets};
use crate::file::Record;
use crate::write::Struct;

const VERTEX_GROUPS: i64 = 1 << 0;
const BEVEL_WEIGHT_VERTEX: i64 = 1 << 3;
const SHARP_EDGE: i64 = 1 << 8;
const SEAM: i64 = 1 << 9;
const CREASE: i64 = 1 << 10;
const BEVEL_WEIGHT_EDGE: i64 = 1 << 11;
const COLOR_VERTEX: i64 = (1 << 16) | (1 << 27);
const CUSTOM_NORMAL: i64 = 1 << 17;
const UV: i64 = 1 << 24;
const SHARP_FACE: i64 = 1 << 25;
const COLOR_CORNER: i64 = (1 << 28) | (1 << 29);

const OBJECT_TRANSFORM: i64 = 1 << 0;
const MAX_DISTANCE: i64 = 1 << 1;
const INVERT_GROUP: i64 = 1 << 2;
const USE_VERTEX: i64 = 1 << 28;
const USE_EDGE: i64 = 1 << 29;
const USE_CORNER: i64 = 1 << 30;
const USE_FACE: i64 = 1 << 31;

const MODE_VERTEX: i64 = 1 << 24;
const MODE_EDGE: i64 = 1 << 25;
const MODE_CORNER: i64 = 1 << 26;
const MODE_FACE: i64 = 1 << 27;
const MODE_TOPOLOGY: i64 = MODE_VERTEX | MODE_EDGE | MODE_CORNER | MODE_FACE;
const USE_VERT: i64 = 1 << 4;
const USE_EDGE_ELEMENT: i64 = 1 << 5;
const USE_LOOP: i64 = 1 << 6;
const USE_POLY: i64 = 1 << 7;
const NEAREST: i64 = 1 << 8;
const NORMAL_PROJECTION: i64 = 1 << 9;
const INTERPOLATE: i64 = 1 << 10;
const NORMAL: i64 = 1 << 11;

const MULTILAYER_VERTEX_GROUPS: usize = 0;
const MULTILAYER_CORNER_COLORS: usize = 2;
const MULTILAYER_UVS: usize = 3;
const MULTILAYER_VERTEX_COLORS: usize = 4;

pub(super) fn read<'a>(modifier: &Record<'a>, refs: &dyn References<'a>) -> DataTransfer {
    let types = modifier.int("data_types").unwrap_or(0);
    let flags = modifier.int("flags").unwrap_or(OBJECT_TRANSFORM) & 0xFFFF_FFFF;
    let has = |bits: i64| types & bits != 0;
    let sources = modifier
        .floats::<5>("layers_select_src")
        .unwrap_or([-2.0; 5]);
    let destinations = modifier
        .floats::<5>("layers_select_dst")
        .unwrap_or([-2.0; 5]);
    let selection = |index: usize| LayerSelection {
        source: match sources[index] as i64 {
            -1 => LayersSource::Active,
            -257 => LayersSource::BoneSelected,
            -258 => LayersSource::BoneDeform,
            value if value >= 0 => LayersSource::Layer(value as u32),
            _ => LayersSource::All,
        },
        destination: match destinations[index] as i64 {
            -1 => LayersDestination::Active,
            -3 => LayersDestination::Index,
            value if value >= 0 => LayersDestination::Layer(value as u32),
            _ => LayersDestination::Name,
        },
    };
    let mut vertex = VertexData::empty();
    vertex.set(VertexData::VERTEX_GROUPS, has(VERTEX_GROUPS));
    vertex.set(VertexData::BEVEL_WEIGHT, has(BEVEL_WEIGHT_VERTEX));
    vertex.set(VertexData::COLOR, has(COLOR_VERTEX));
    let mut edge = EdgeData::empty();
    edge.set(EdgeData::SHARP, has(SHARP_EDGE));
    edge.set(EdgeData::SEAM, has(SEAM));
    edge.set(EdgeData::CREASE, has(CREASE));
    edge.set(EdgeData::BEVEL_WEIGHT, has(BEVEL_WEIGHT_EDGE));
    let mut corner = CornerData::empty();
    corner.set(CornerData::CUSTOM_NORMAL, has(CUSTOM_NORMAL));
    corner.set(CornerData::UV, has(UV));
    corner.set(CornerData::COLOR, has(COLOR_CORNER));
    let mut face = FaceData::empty();
    face.set(FaceData::SMOOTH, has(SHARP_FACE));
    let mode = |name: &str, default: i64| modifier.int(name).unwrap_or(default);
    DataTransfer {
        object: refs.object(modifier.id("ob_source")),
        use_object_transform: flags & OBJECT_TRANSFORM != 0,
        use_vert_data: flags & USE_VERTEX != 0 || !vertex.is_empty(),
        data_types_verts: vertex,
        vert_mapping: vertex_mapping(mode("vmap_mode", MODE_VERTEX | USE_VERT | NEAREST)),
        use_edge_data: flags & USE_EDGE != 0 || !edge.is_empty(),
        data_types_edges: edge,
        edge_mapping: edge_mapping(mode("emap_mode", MODE_EDGE | USE_EDGE_ELEMENT | NEAREST)),
        use_loop_data: flags & USE_CORNER != 0 || !corner.is_empty(),
        data_types_loops: corner,
        loop_mapping: corner_mapping(mode(
            "lmap_mode",
            MODE_CORNER | USE_POLY | USE_VERT | NEAREST | NORMAL,
        )),
        use_poly_data: flags & USE_FACE != 0 || !face.is_empty(),
        data_types_polys: face,
        poly_mapping: face_mapping(mode("pmap_mode", MODE_FACE | USE_POLY | NEAREST)),
        use_max_distance: flags & MAX_DISTANCE != 0,
        max_distance: modifier.float("map_max_distance").unwrap_or(1.0).max(0.0),
        ray_radius: modifier.float("map_ray_radius").unwrap_or(0.0).max(0.0),
        islands_precision: modifier
            .float("islands_precision")
            .unwrap_or(0.0)
            .clamp(0.0, 1.0),
        layers_vgroup: selection(MULTILAYER_VERTEX_GROUPS),
        layers_vcol_vert: selection(MULTILAYER_VERTEX_COLORS),
        layers_vcol_loop: selection(MULTILAYER_CORNER_COLORS),
        layers_uv: selection(MULTILAYER_UVS),
        mix_mode: match modifier.int("mix_mode").unwrap_or(0) {
            1 => MixMode::ReplaceAboveThreshold,
            2 => MixMode::ReplaceBelowThreshold,
            16 => MixMode::Mix,
            17 => MixMode::Add,
            18 => MixMode::Subtract,
            19 => MixMode::Multiply,
            _ => MixMode::Replace,
        },
        mix_factor: modifier.float("mix_factor").unwrap_or(1.0).clamp(0.0, 1.0),
        vertex_group: modifier.text("defgrp_name").unwrap_or_default(),
        invert_vertex_group: flags & INVERT_GROUP != 0,
    }
}

fn vertex_mapping(mode: i64) -> VertexMapping {
    if mode == MODE_TOPOLOGY {
        return VertexMapping::Topology;
    }
    match (
        mode & USE_EDGE_ELEMENT != 0,
        mode & USE_POLY != 0,
        mode & INTERPOLATE != 0,
        mode & NORMAL_PROJECTION != 0,
    ) {
        (true, _, false, _) => VertexMapping::EdgeNearest,
        (true, _, true, _) => VertexMapping::EdgeInterpolatedNearest,
        (false, true, true, true) => VertexMapping::FaceInterpolatedNormalProjected,
        (false, true, true, false) => VertexMapping::FaceInterpolatedNearest,
        (false, true, false, _) => VertexMapping::FaceNearest,
        _ => VertexMapping::Nearest,
    }
}

fn edge_mapping(mode: i64) -> EdgeMapping {
    if mode == MODE_TOPOLOGY {
        return EdgeMapping::Topology;
    }
    if mode & NORMAL_PROJECTION != 0 {
        return EdgeMapping::EdgeInterpolatedNormalProjected;
    }
    match (mode & USE_VERT != 0, mode & USE_POLY != 0) {
        (true, _) => EdgeMapping::VertexNearest,
        (false, true) => EdgeMapping::FaceNearest,
        (false, false) => EdgeMapping::Nearest,
    }
}

fn corner_mapping(mode: i64) -> CornerMapping {
    if mode == MODE_TOPOLOGY {
        return CornerMapping::Topology;
    }
    if mode & NORMAL_PROJECTION != 0 {
        return CornerMapping::FaceInterpolatedNormalProjected;
    }
    match (
        mode & USE_LOOP != 0,
        mode & USE_VERT != 0,
        mode & INTERPOLATE != 0,
    ) {
        (true, _, _) => CornerMapping::NearestCornerNormal,
        (false, true, _) => CornerMapping::NearestFaceNormal,
        (false, false, true) => CornerMapping::FaceInterpolatedNearest,
        (false, false, false) => CornerMapping::FaceNearest,
    }
}

fn face_mapping(mode: i64) -> FaceMapping {
    if mode == MODE_TOPOLOGY {
        return FaceMapping::Topology;
    }
    match (mode & NORMAL_PROJECTION != 0, mode & NORMAL != 0) {
        (true, _) => FaceMapping::FaceInterpolatedNormalProjected,
        (false, true) => FaceMapping::Normal,
        (false, false) => FaceMapping::Nearest,
    }
}

pub(super) fn write(transfer: &DataTransfer, record: &mut Struct<'_>, targets: &dyn Targets) {
    let mut types = 0;
    let mut add = |on: bool, bits: i64| {
        if on {
            types |= bits;
        }
    };
    add(
        transfer
            .data_types_verts
            .contains(VertexData::VERTEX_GROUPS),
        VERTEX_GROUPS,
    );
    add(
        transfer.data_types_verts.contains(VertexData::BEVEL_WEIGHT),
        BEVEL_WEIGHT_VERTEX,
    );
    add(
        transfer.data_types_verts.contains(VertexData::COLOR),
        COLOR_VERTEX,
    );
    add(
        transfer.data_types_edges.contains(EdgeData::SHARP),
        SHARP_EDGE,
    );
    add(transfer.data_types_edges.contains(EdgeData::SEAM), SEAM);
    add(transfer.data_types_edges.contains(EdgeData::CREASE), CREASE);
    add(
        transfer.data_types_edges.contains(EdgeData::BEVEL_WEIGHT),
        BEVEL_WEIGHT_EDGE,
    );
    add(
        transfer
            .data_types_loops
            .contains(CornerData::CUSTOM_NORMAL),
        CUSTOM_NORMAL,
    );
    add(transfer.data_types_loops.contains(CornerData::UV), UV);
    add(
        transfer.data_types_loops.contains(CornerData::COLOR),
        COLOR_CORNER,
    );
    add(
        transfer.data_types_polys.contains(FaceData::SMOOTH),
        SHARP_FACE,
    );
    let mut sources: Vec<i64> = vec![-2; 5];
    let mut destinations: Vec<i64> = vec![-2; 5];
    for (index, selection) in [
        (MULTILAYER_VERTEX_GROUPS, transfer.layers_vgroup),
        (MULTILAYER_CORNER_COLORS, transfer.layers_vcol_loop),
        (MULTILAYER_UVS, transfer.layers_uv),
        (MULTILAYER_VERTEX_COLORS, transfer.layers_vcol_vert),
    ] {
        sources[index] = match selection.source {
            LayersSource::Active => -1,
            LayersSource::All => -2,
            LayersSource::Layer(layer) => i64::from(layer),
            LayersSource::BoneSelected => -257,
            LayersSource::BoneDeform => -258,
        };
        destinations[index] = match selection.destination {
            LayersDestination::Active => -1,
            LayersDestination::Name => -2,
            LayersDestination::Index => -3,
            LayersDestination::Layer(layer) => i64::from(layer),
        };
    }
    record
        .set_pointer("ob_source", targets.object(transfer.object))
        .set_int("data_types", types)
        .set_flag("flags", OBJECT_TRANSFORM, transfer.use_object_transform)
        .set_flag("flags", USE_VERTEX, transfer.use_vert_data)
        .set_flag("flags", USE_EDGE, transfer.use_edge_data)
        .set_flag("flags", USE_CORNER, transfer.use_loop_data)
        .set_flag("flags", USE_FACE, transfer.use_poly_data)
        .set_flag("flags", MAX_DISTANCE, transfer.use_max_distance)
        .set_flag("flags", INVERT_GROUP, transfer.invert_vertex_group)
        .set_int("vmap_mode", vertex_mode(transfer.vert_mapping))
        .set_int("emap_mode", edge_mode(transfer.edge_mapping))
        .set_int("lmap_mode", corner_mode(transfer.loop_mapping))
        .set_int("pmap_mode", face_mode(transfer.poly_mapping))
        .set_float("map_max_distance", transfer.max_distance)
        .set_float("map_ray_radius", transfer.ray_radius)
        .set_float("islands_precision", transfer.islands_precision)
        .set_ints("layers_select_src", &sources)
        .set_ints("layers_select_dst", &destinations)
        .set_int(
            "mix_mode",
            match transfer.mix_mode {
                MixMode::Replace => 0,
                MixMode::ReplaceAboveThreshold => 1,
                MixMode::ReplaceBelowThreshold => 2,
                MixMode::Mix => 16,
                MixMode::Add => 17,
                MixMode::Subtract => 18,
                MixMode::Multiply => 19,
            },
        )
        .set_float("mix_factor", transfer.mix_factor)
        .set_text("defgrp_name", &transfer.vertex_group);
}

fn vertex_mode(mapping: VertexMapping) -> i64 {
    match mapping {
        VertexMapping::Topology => MODE_TOPOLOGY,
        VertexMapping::Nearest => MODE_VERTEX | USE_VERT | NEAREST,
        VertexMapping::EdgeNearest => MODE_VERTEX | USE_EDGE_ELEMENT | NEAREST,
        VertexMapping::EdgeInterpolatedNearest => {
            MODE_VERTEX | USE_EDGE_ELEMENT | NEAREST | INTERPOLATE
        }
        VertexMapping::FaceNearest => MODE_VERTEX | USE_POLY | NEAREST,
        VertexMapping::FaceInterpolatedNearest => MODE_VERTEX | USE_POLY | NEAREST | INTERPOLATE,
        VertexMapping::FaceInterpolatedNormalProjected => {
            MODE_VERTEX | USE_POLY | NORMAL_PROJECTION | INTERPOLATE
        }
    }
}

fn edge_mode(mapping: EdgeMapping) -> i64 {
    match mapping {
        EdgeMapping::Topology => MODE_TOPOLOGY,
        EdgeMapping::VertexNearest => MODE_EDGE | USE_VERT | NEAREST,
        EdgeMapping::Nearest => MODE_EDGE | USE_EDGE_ELEMENT | NEAREST,
        EdgeMapping::FaceNearest => MODE_EDGE | USE_POLY | NEAREST,
        EdgeMapping::EdgeInterpolatedNormalProjected => {
            MODE_EDGE | USE_VERT | NORMAL_PROJECTION | INTERPOLATE
        }
    }
}

fn corner_mode(mapping: CornerMapping) -> i64 {
    match mapping {
        CornerMapping::Topology => MODE_TOPOLOGY,
        CornerMapping::NearestCornerNormal => MODE_CORNER | USE_LOOP | USE_VERT | NEAREST | NORMAL,
        CornerMapping::NearestFaceNormal => MODE_CORNER | USE_POLY | USE_VERT | NEAREST | NORMAL,
        CornerMapping::FaceNearest => MODE_CORNER | USE_POLY | NEAREST,
        CornerMapping::FaceInterpolatedNearest => MODE_CORNER | USE_POLY | NEAREST | INTERPOLATE,
        CornerMapping::FaceInterpolatedNormalProjected => {
            MODE_CORNER | USE_POLY | NORMAL_PROJECTION | INTERPOLATE
        }
    }
}

fn face_mode(mapping: FaceMapping) -> i64 {
    match mapping {
        FaceMapping::Topology => MODE_TOPOLOGY,
        FaceMapping::Nearest => MODE_FACE | USE_POLY | NEAREST,
        FaceMapping::Normal => MODE_FACE | USE_POLY | NORMAL,
        FaceMapping::FaceInterpolatedNormalProjected => {
            MODE_FACE | USE_POLY | NORMAL_PROJECTION | INTERPOLATE
        }
    }
}
