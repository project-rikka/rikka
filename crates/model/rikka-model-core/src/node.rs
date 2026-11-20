use crate::camera::CameraId;
use crate::id::model_id;
use crate::light::LightId;
use crate::mesh::MeshId;
use crate::skin::SkinId;
use crate::transform::Transform;

model_id!(NodeId);

#[derive(Clone, Debug, Default)]
#[must_use]
pub struct Node {
    pub name: String,
    pub transform: Transform,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub mesh: Option<MeshId>,
    pub skin: Option<SkinId>,
    pub camera: Option<CameraId>,
    pub light: Option<LightId>,
    pub morph_weights: Vec<f32>,
}

impl Node {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    pub fn mesh(mut self, mesh: MeshId) -> Self {
        self.mesh = Some(mesh);
        self
    }

    pub fn skin(mut self, skin: SkinId) -> Self {
        self.skin = Some(skin);
        self
    }

    pub fn camera(mut self, camera: CameraId) -> Self {
        self.camera = Some(camera);
        self
    }

    pub fn light(mut self, light: LightId) -> Self {
        self.light = Some(light);
        self
    }

    pub fn morph_weights(mut self, weights: impl Into<Vec<f32>>) -> Self {
        self.morph_weights = weights.into();
        self
    }
}
