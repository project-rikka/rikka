use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub(crate) mod khr;

pub(crate) type Index = u32;

pub(crate) const VERSION: &str = "2.0";

pub(crate) const TRIANGLES: u32 = 4;

pub(crate) const TRIANGLE_STRIP: u32 = 5;

pub(crate) const TRIANGLE_FAN: u32 = 6;

pub(crate) const BYTE: u32 = 5120;

pub(crate) const UNSIGNED_BYTE: u32 = 5121;

pub(crate) const SHORT: u32 = 5122;

pub(crate) const UNSIGNED_SHORT: u32 = 5123;

pub(crate) const UNSIGNED_INT: u32 = 5125;

pub(crate) const FLOAT: u32 = 5126;

pub(crate) const NEAREST: u32 = 9728;

pub(crate) const LINEAR: u32 = 9729;

pub(crate) const LINEAR_MIPMAP_LINEAR: u32 = 9987;

pub(crate) const REPEAT: u32 = 10497;

pub(crate) const CLAMP_TO_EDGE: u32 = 33071;

pub(crate) const MIRRORED_REPEAT: u32 = 33648;

pub(crate) const ARRAY_BUFFER: u32 = 34962;

pub(crate) const ELEMENT_ARRAY_BUFFER: u32 = 34963;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Document {
    pub(crate) asset: Asset,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) extensions_used: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) extensions_required: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) accessors: Vec<Accessor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) animations: Vec<Animation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) buffers: Vec<Buffer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) buffer_views: Vec<BufferView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) cameras: Vec<Camera>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) images: Vec<Image>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) materials: Vec<Material>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) meshes: Vec<Mesh>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) nodes: Vec<Node>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) samplers: Vec<Sampler>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) scene: Option<Index>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) scenes: Vec<Scene>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) skins: Vec<Skin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) textures: Vec<Texture>,
    #[serde(default, skip_serializing_if = "DocumentExtensions::is_empty")]
    pub(crate) extensions: DocumentExtensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Asset {
    pub(crate) version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) min_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) generator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) copyright: Option<String>,
}

impl Default for Asset {
    fn default() -> Self {
        Self {
            version: VERSION.to_owned(),
            min_version: None,
            generator: None,
            copyright: None,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Buffer {
    pub(crate) byte_length: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BufferView {
    pub(crate) buffer: Index,
    #[serde(default, skip_serializing_if = "is_default")]
    pub(crate) byte_offset: usize,
    pub(crate) byte_length: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) byte_stride: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) target: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum Shape {
    #[default]
    Scalar,
    Vec2,
    Vec3,
    Vec4,
    Mat2,
    Mat3,
    Mat4,
}

impl Shape {
    pub(crate) fn components(self) -> usize {
        match self {
            Self::Scalar => 1,
            Self::Vec2 => 2,
            Self::Vec3 => 3,
            Self::Vec4 | Self::Mat2 => 4,
            Self::Mat3 => 9,
            Self::Mat4 => 16,
        }
    }

    pub(crate) fn columns(self) -> usize {
        match self {
            Self::Mat2 => 2,
            Self::Mat3 => 3,
            Self::Mat4 => 4,
            _ => 1,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Accessor {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) buffer_view: Option<Index>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub(crate) byte_offset: usize,
    pub(crate) component_type: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub(crate) normalized: bool,
    pub(crate) count: usize,
    #[serde(rename = "type")]
    pub(crate) shape: Shape,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) min: Vec<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) max: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sparse: Option<Sparse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Sparse {
    pub(crate) count: usize,
    pub(crate) indices: SparseIndices,
    pub(crate) values: SparseValues,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SparseIndices {
    pub(crate) buffer_view: Index,
    #[serde(default, skip_serializing_if = "is_default")]
    pub(crate) byte_offset: usize,
    pub(crate) component_type: u32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SparseValues {
    pub(crate) buffer_view: Index,
    #[serde(default, skip_serializing_if = "is_default")]
    pub(crate) byte_offset: usize,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Mesh {
    pub(crate) primitives: Vec<Primitive>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) weights: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) extras: Option<MeshExtras>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MeshExtras {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) target_names: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Primitive {
    pub(crate) attributes: BTreeMap<String, Index>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) indices: Option<Index>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) material: Option<Index>,
    #[serde(default = "triangles", skip_serializing_if = "is_triangles")]
    pub(crate) mode: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) targets: Vec<BTreeMap<String, Index>>,
}

impl Default for Primitive {
    fn default() -> Self {
        Self {
            attributes: BTreeMap::new(),
            indices: None,
            material: None,
            mode: triangles(),
            targets: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Node {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) children: Vec<Index>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) matrix: Option<[f32; 16]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) translation: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) rotation: Option<[f32; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) scale: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) mesh: Option<Index>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) skin: Option<Index>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) camera: Option<Index>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) weights: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(default, skip_serializing_if = "NodeExtensions::is_empty")]
    pub(crate) extensions: NodeExtensions,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Scene {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) nodes: Vec<Index>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Skin {
    pub(crate) joints: Vec<Index>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) inverse_bind_matrices: Option<Index>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) skeleton: Option<Index>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Animation {
    pub(crate) channels: Vec<Channel>,
    pub(crate) samplers: Vec<AnimationSampler>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Channel {
    pub(crate) sampler: Index,
    pub(crate) target: ChannelTarget,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelTarget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) node: Option<Index>,
    pub(crate) path: Path,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Path {
    #[default]
    Translation,
    Rotation,
    Scale,
    Weights,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnimationSampler {
    pub(crate) input: Index,
    pub(crate) output: Index,
    #[serde(default, skip_serializing_if = "is_default")]
    pub(crate) interpolation: Interpolation,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum Interpolation {
    #[default]
    Linear,
    Step,
    CubicSpline,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Material {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) pbr_metallic_roughness: Option<PbrMetallicRoughness>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) normal_texture: Option<NormalTextureInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) occlusion_texture: Option<OcclusionTextureInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) emissive_texture: Option<TextureInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) emissive_factor: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub(crate) alpha_mode: AlphaMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) alpha_cutoff: Option<f32>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub(crate) double_sided: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(default, skip_serializing_if = "MaterialExtensions::is_empty")]
    pub(crate) extensions: MaterialExtensions,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum AlphaMode {
    #[default]
    Opaque,
    Mask,
    Blend,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PbrMetallicRoughness {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) base_color_factor: Option<[f32; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) base_color_texture: Option<TextureInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) metallic_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) roughness_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) metallic_roughness_texture: Option<TextureInfo>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TextureInfo {
    pub(crate) index: Index,
    #[serde(default, skip_serializing_if = "is_default")]
    pub(crate) tex_coord: u32,
    #[serde(default, skip_serializing_if = "TextureInfoExtensions::is_empty")]
    pub(crate) extensions: TextureInfoExtensions,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NormalTextureInfo {
    pub(crate) index: Index,
    #[serde(default, skip_serializing_if = "is_default")]
    pub(crate) tex_coord: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) scale: Option<f32>,
    #[serde(default, skip_serializing_if = "TextureInfoExtensions::is_empty")]
    pub(crate) extensions: TextureInfoExtensions,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OcclusionTextureInfo {
    pub(crate) index: Index,
    #[serde(default, skip_serializing_if = "is_default")]
    pub(crate) tex_coord: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) strength: Option<f32>,
    #[serde(default, skip_serializing_if = "TextureInfoExtensions::is_empty")]
    pub(crate) extensions: TextureInfoExtensions,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Texture {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) source: Option<Index>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sampler: Option<Index>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Image {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) buffer_view: Option<Index>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Sampler {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) mag_filter: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) min_filter: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) wrap_s: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) wrap_t: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Camera {
    #[serde(rename = "type")]
    pub(crate) kind: CameraKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) perspective: Option<Perspective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) orthographic: Option<Orthographic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CameraKind {
    #[default]
    Perspective,
    Orthographic,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Perspective {
    pub(crate) yfov: f32,
    pub(crate) znear: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) aspect_ratio: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) zfar: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Orthographic {
    pub(crate) xmag: f32,
    pub(crate) ymag: f32,
    pub(crate) znear: f32,
    pub(crate) zfar: f32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct DocumentExtensions {
    #[serde(
        rename = "KHR_lights_punctual",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) lights_punctual: Option<LightsPunctual>,
    #[serde(flatten)]
    pub(crate) other: Map<String, Value>,
}

impl DocumentExtensions {
    pub(crate) fn is_empty(&self) -> bool {
        self.lights_punctual.is_none() && self.other.is_empty()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct LightsPunctual {
    pub(crate) lights: Vec<Light>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Light {
    #[serde(rename = "type")]
    pub(crate) kind: LightKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) color: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) intensity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) range: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) spot: Option<Spot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LightKind {
    #[default]
    Directional,
    Point,
    Spot,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Spot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) inner_cone_angle: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) outer_cone_angle: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct NodeExtensions {
    #[serde(
        rename = "KHR_lights_punctual",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) lights_punctual: Option<NodeLight>,
    #[serde(flatten)]
    pub(crate) other: Map<String, Value>,
}

impl NodeExtensions {
    pub(crate) fn is_empty(&self) -> bool {
        self.lights_punctual.is_none() && self.other.is_empty()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct NodeLight {
    pub(crate) light: Index,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct MaterialExtensions {
    #[serde(
        rename = "KHR_materials_emissive_strength",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) emissive_strength: Option<EmissiveStrength>,
    #[serde(
        rename = "KHR_materials_unlit",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) unlit: Option<Map<String, Value>>,
    #[serde(
        rename = "KHR_materials_clearcoat",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) clearcoat: Option<khr::Clearcoat>,
    #[serde(
        rename = "KHR_materials_sheen",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) sheen: Option<khr::Sheen>,
    #[serde(
        rename = "KHR_materials_transmission",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) transmission: Option<khr::Transmission>,
    #[serde(
        rename = "KHR_materials_volume",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) volume: Option<khr::Volume>,
    #[serde(
        rename = "KHR_materials_ior",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) ior: Option<khr::Ior>,
    #[serde(
        rename = "KHR_materials_specular",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) specular: Option<khr::Specular>,
    #[serde(
        rename = "KHR_materials_iridescence",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) iridescence: Option<khr::Iridescence>,
    #[serde(
        rename = "KHR_materials_anisotropy",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) anisotropy: Option<khr::Anisotropy>,
    #[serde(
        rename = "KHR_materials_dispersion",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) dispersion: Option<khr::Dispersion>,
    #[serde(flatten)]
    pub(crate) other: Map<String, Value>,
}

impl MaterialExtensions {
    pub(crate) fn is_empty(&self) -> bool {
        self.emissive_strength.is_none()
            && self.unlit.is_none()
            && self.clearcoat.is_none()
            && self.sheen.is_none()
            && self.transmission.is_none()
            && self.volume.is_none()
            && self.ior.is_none()
            && self.specular.is_none()
            && self.iridescence.is_none()
            && self.anisotropy.is_none()
            && self.dispersion.is_none()
            && self.other.is_empty()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EmissiveStrength {
    pub(crate) emissive_strength: f32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct TextureInfoExtensions {
    #[serde(
        rename = "KHR_texture_transform",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) texture_transform: Option<TextureTransform>,
    #[serde(flatten)]
    pub(crate) other: Map<String, Value>,
}

impl TextureInfoExtensions {
    pub(crate) fn is_empty(&self) -> bool {
        self.texture_transform.is_none() && self.other.is_empty()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TextureTransform {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) offset: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) rotation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) scale: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) tex_coord: Option<u32>,
}
fn triangles() -> u32 {
    TRIANGLES
}

fn is_triangles(mode: &u32) -> bool {
    *mode == TRIANGLES
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}
