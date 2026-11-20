use std::sync::{Arc, OnceLock};

use glam::{Mat3, Mat4, Vec3};
use rikka_humanoid::Anatomy;
use rikka_mmd::PmxImporter;
use rikka_model_core::{Import, Model, NodeId, Registry};
use rikka_physics_tailor::{Budget, ClothAsset, ClothSettings};
use rikka_vrm::VrmImporter;

const MIKU: &[u8] = include_bytes!("../../../../model/rikka-mmd/assets/mmd/miku.pmx");
const AVATAR: &[u8] = include_bytes!("../../../../model/rikka-vrm/assets/avatar.vrm");

fn read(bytes: &[u8], name: &str) -> Model {
    Import::named_bytes(
        name,
        bytes.to_vec(),
        Arc::new(
            Registry::new()
                .with_importer(PmxImporter)
                .with_importer(VrmImporter),
        ),
    )
    .run()
    .unwrap_or_else(|error| panic!("{name} cannot be read: {error}"))
}

pub(crate) fn miku() -> &'static Model {
    static MODEL: OnceLock<Model> = OnceLock::new();
    MODEL.get_or_init(|| read(MIKU, "miku.pmx"))
}

pub(crate) fn avatar() -> &'static Model {
    static MODEL: OnceLock<Model> = OnceLock::new();
    MODEL.get_or_init(|| read(AVATAR, "avatar.vrm"))
}

pub(crate) fn miku_asset() -> &'static ClothAsset {
    static ASSET: OnceLock<ClothAsset> = OnceLock::new();
    ASSET.get_or_init(|| build_asset(miku()))
}

pub(crate) fn avatar_asset() -> &'static ClothAsset {
    static ASSET: OnceLock<ClothAsset> = OnceLock::new();
    ASSET.get_or_init(|| build_asset(avatar()))
}

pub(crate) fn build_asset(model: &Model) -> ClothAsset {
    ClothAsset::build(
        model,
        &Anatomy::recognize(model),
        &ClothSettings::new(),
        Budget::default(),
    )
    .expect("sample builds a cloth asset")
}

pub(crate) struct RestVertex {
    pub(crate) position: Vec3,
    pub(crate) normal: Vec3,
}

pub(crate) struct Rest<'m> {
    model: &'m Model,
    worlds: Vec<Mat4>,
}

impl<'m> Rest<'m> {
    pub(crate) fn new(model: &'m Model) -> Rest<'m> {
        Rest {
            model,
            worlds: model.world_transforms(),
        }
    }

    pub(crate) fn vertex(&self, node: NodeId, vertex: u32) -> RestVertex {
        let described = self.model.node(node);
        let mesh = self.model.mesh(described.mesh.expect("a mesh node"));
        let vertex = vertex as usize;
        let matrix = match described.skin.filter(|_| mesh.vertices.is_skinned()) {
            Some(skin) => {
                let skin = self.model.skin(skin);
                let influences = mesh.vertices.influences(vertex).expect("skin weights");
                let usable: Vec<(usize, f32)> = influences
                    .iter()
                    .map(|(slot, weight)| (usize::from(*slot), *weight))
                    .filter(|(slot, weight)| *weight > 0.0 && *slot < skin.joints.len())
                    .collect();
                let total: f32 = usable.iter().map(|(_, weight)| weight).sum();
                usable.iter().fold(Mat4::ZERO, |sum, (slot, weight)| {
                    sum + self.worlds[skin.joints[*slot].index()]
                        * skin.inverse_bind[*slot]
                        * (*weight / total)
                })
            }
            None => self.worlds[node.index()],
        };
        RestVertex {
            position: matrix.transform_point3(mesh.vertices.positions[vertex]),
            normal: mesh
                .vertices
                .normals
                .get(vertex)
                .map_or(Vec3::ZERO, |normal| {
                    (Mat3::from_mat4(matrix) * *normal).normalize_or_zero()
                }),
        }
    }
}
