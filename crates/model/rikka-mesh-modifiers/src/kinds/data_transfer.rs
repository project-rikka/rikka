use bitflags::bitflags;
use rikka_file_format::{Codec, DecodeError, Reader, Writer};

use crate::codec::{flags_codec, non_negative, record, tagged, within};
use crate::reference::ObjectRef;

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct VertexData: u8 {
        const VERTEX_GROUPS = 1 << 0;
        const BEVEL_WEIGHT = 1 << 1;
        const COLOR = 1 << 2;
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct EdgeData: u8 {
        const SHARP = 1 << 0;
        const SEAM = 1 << 1;
        const CREASE = 1 << 2;
        const BEVEL_WEIGHT = 1 << 3;
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct CornerData: u8 {
        const CUSTOM_NORMAL = 1 << 0;
        const COLOR = 1 << 1;
        const UV = 1 << 2;
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct FaceData: u8 {
        const SMOOTH = 1 << 0;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum VertexMapping {
    Topology,
    #[default]
    Nearest,
    EdgeNearest,
    EdgeInterpolatedNearest,
    FaceNearest,
    FaceInterpolatedNearest,
    FaceInterpolatedNormalProjected,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EdgeMapping {
    Topology,
    VertexNearest,
    #[default]
    Nearest,
    FaceNearest,
    EdgeInterpolatedNormalProjected,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CornerMapping {
    Topology,
    NearestCornerNormal,
    #[default]
    NearestFaceNormal,
    FaceNearest,
    FaceInterpolatedNearest,
    FaceInterpolatedNormalProjected,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FaceMapping {
    Topology,
    #[default]
    Nearest,
    Normal,
    FaceInterpolatedNormalProjected,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MixMode {
    #[default]
    Replace,
    ReplaceAboveThreshold,
    ReplaceBelowThreshold,
    Mix,
    Add,
    Subtract,
    Multiply,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LayersSource {
    Active,
    #[default]
    All,
    Layer(u32),
    BoneSelected,
    BoneDeform,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LayersDestination {
    Active,
    #[default]
    Name,
    Index,
    Layer(u32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LayerSelection {
    pub source: LayersSource,
    pub destination: LayersDestination,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataTransfer {
    pub object: Option<ObjectRef>,
    pub use_object_transform: bool,
    pub use_vert_data: bool,
    pub data_types_verts: VertexData,
    pub vert_mapping: VertexMapping,
    pub use_edge_data: bool,
    pub data_types_edges: EdgeData,
    pub edge_mapping: EdgeMapping,
    pub use_loop_data: bool,
    pub data_types_loops: CornerData,
    pub loop_mapping: CornerMapping,
    pub use_poly_data: bool,
    pub data_types_polys: FaceData,
    pub poly_mapping: FaceMapping,
    pub use_max_distance: bool,
    pub max_distance: f32,
    pub ray_radius: f32,
    pub islands_precision: f32,
    pub layers_vgroup: LayerSelection,
    pub layers_vcol_vert: LayerSelection,
    pub layers_vcol_loop: LayerSelection,
    pub layers_uv: LayerSelection,
    pub mix_mode: MixMode,
    pub mix_factor: f32,
    pub vertex_group: String,
    pub invert_vertex_group: bool,
}

impl Default for DataTransfer {
    fn default() -> DataTransfer {
        DataTransfer {
            object: None,
            use_object_transform: true,
            use_vert_data: false,
            data_types_verts: VertexData::empty(),
            vert_mapping: VertexMapping::Nearest,
            use_edge_data: false,
            data_types_edges: EdgeData::empty(),
            edge_mapping: EdgeMapping::Nearest,
            use_loop_data: false,
            data_types_loops: CornerData::empty(),
            loop_mapping: CornerMapping::NearestFaceNormal,
            use_poly_data: false,
            data_types_polys: FaceData::empty(),
            poly_mapping: FaceMapping::Nearest,
            use_max_distance: false,
            max_distance: 1.0,
            ray_radius: 0.0,
            islands_precision: 0.0,
            layers_vgroup: LayerSelection::default(),
            layers_vcol_vert: LayerSelection::default(),
            layers_vcol_loop: LayerSelection::default(),
            layers_uv: LayerSelection::default(),
            mix_mode: MixMode::Replace,
            mix_factor: 1.0,
            vertex_group: String::new(),
            invert_vertex_group: false,
        }
    }
}

flags_codec!(VertexData, u8, "vertex data type");
flags_codec!(EdgeData, u8, "edge data type");
flags_codec!(CornerData, u8, "face corner data type");
flags_codec!(FaceData, u8, "face data type");

tagged!(VertexMapping, "vertex mapping" {
    Topology = 0,
    Nearest = 1,
    EdgeNearest = 2,
    EdgeInterpolatedNearest = 3,
    FaceNearest = 4,
    FaceInterpolatedNearest = 5,
    FaceInterpolatedNormalProjected = 6,
});

tagged!(EdgeMapping, "edge mapping" {
    Topology = 0,
    VertexNearest = 1,
    Nearest = 2,
    FaceNearest = 3,
    EdgeInterpolatedNormalProjected = 4,
});

tagged!(CornerMapping, "face corner mapping" {
    Topology = 0,
    NearestCornerNormal = 1,
    NearestFaceNormal = 2,
    FaceNearest = 3,
    FaceInterpolatedNearest = 4,
    FaceInterpolatedNormalProjected = 5,
});

tagged!(FaceMapping, "face mapping" {
    Topology = 0,
    Nearest = 1,
    Normal = 2,
    FaceInterpolatedNormalProjected = 3,
});

tagged!(MixMode, "mix mode" {
    Replace = 0,
    ReplaceAboveThreshold = 1,
    ReplaceBelowThreshold = 2,
    Mix = 3,
    Add = 4,
    Subtract = 5,
    Multiply = 6,
});

impl Codec for LayersSource {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        match self {
            LayersSource::Active => out.tag(0),
            LayersSource::All => out.tag(1),
            LayersSource::Layer(index) => {
                out.tag(2);
                out.put(index);
            }
            LayersSource::BoneSelected => out.tag(3),
            LayersSource::BoneDeform => out.tag(4),
        }
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match input.tag()? {
            0 => Ok(LayersSource::Active),
            1 => Ok(LayersSource::All),
            2 => input.get().map(LayersSource::Layer),
            3 => Ok(LayersSource::BoneSelected),
            4 => Ok(LayersSource::BoneDeform),
            _ => Err(DecodeError::Malformed("unknown source layer selection")),
        }
    }
}

impl Codec for LayersDestination {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        match self {
            LayersDestination::Active => out.tag(0),
            LayersDestination::Name => out.tag(1),
            LayersDestination::Index => out.tag(2),
            LayersDestination::Layer(index) => {
                out.tag(3);
                out.put(index);
            }
        }
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match input.tag()? {
            0 => Ok(LayersDestination::Active),
            1 => Ok(LayersDestination::Name),
            2 => Ok(LayersDestination::Index),
            3 => input.get().map(LayersDestination::Layer),
            _ => Err(DecodeError::Malformed(
                "unknown destination layer selection",
            )),
        }
    }
}

record!(LayerSelection {
    source,
    destination,
});

record!(DataTransfer {
    object,
    use_object_transform,
    use_vert_data,
    data_types_verts,
    vert_mapping,
    use_edge_data,
    data_types_edges,
    edge_mapping,
    use_loop_data,
    data_types_loops,
    loop_mapping,
    use_poly_data,
    data_types_polys,
    poly_mapping,
    use_max_distance,
    max_distance => non_negative("data transfer maximum distance is negative"),
    ray_radius => non_negative("data transfer ray radius is negative"),
    islands_precision => within(0.0, 1.0, "data transfer islands precision is outside 0..1"),
    layers_vgroup,
    layers_vcol_vert,
    layers_vcol_loop,
    layers_uv,
    mix_mode,
    mix_factor => within(0.0, 1.0, "data transfer mix factor is outside 0..1"),
    vertex_group,
    invert_vertex_group,
});
