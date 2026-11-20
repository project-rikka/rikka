use std::path::PathBuf;

use crate::animation::{Animation, AnimationId};
use crate::camera::{Camera, CameraId};
use crate::coordinates::CoordinateSystem;
use crate::error::Result;
use crate::extension::Extension;
use crate::light::{Light, LightId};
use crate::material::{Material, MaterialId};
use crate::mesh::{Mesh, MeshId};
use crate::model::Model;
use crate::node::{Node, NodeId};
use crate::skin::{Skin, SkinId};
use crate::texture::{Image, ImageId, Texture, TextureId};
use crate::warning::Warning;

#[derive(Debug, Default)]
#[must_use]
pub struct ModelBuilder {
    model: Model,
}

impl ModelBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn importer(mut self, name: &'static str) -> Self {
        self.model.metadata.importer = name;
        self
    }

    pub fn generator(mut self, generator: impl Into<String>) -> Self {
        self.model.metadata.generator = Some(generator.into());
        self
    }

    pub fn source(mut self, source: impl Into<PathBuf>) -> Self {
        self.model.metadata.source = Some(source.into());
        self
    }

    pub fn coordinates(mut self, coordinates: CoordinateSystem) -> Self {
        self.model.metadata.coordinates = coordinates;
        self
    }

    pub fn unit_scale(mut self, metres_per_unit: f32) -> Self {
        self.model.metadata.unit_scale = metres_per_unit;
        self
    }

    pub fn add_node(&mut self, node: Node) -> NodeId {
        let parent = node.parent;
        let id = NodeId(push(&mut self.model.nodes, node));
        match parent {
            Some(parent) => self.model.nodes[parent.index()].children.push(id),
            None => self.model.roots.push(id),
        }
        id
    }

    pub fn attach(&mut self, parent: NodeId, child: NodeId) {
        if let Some(previous) = self.model.nodes[child.index()].parent {
            self.model.nodes[previous.index()]
                .children
                .retain(|sibling| *sibling != child);
        }
        self.model.roots.retain(|root| *root != child);
        self.model.nodes[child.index()].parent = Some(parent);
        self.model.nodes[parent.index()].children.push(child);
    }

    pub fn add_mesh(&mut self, mesh: Mesh) -> MeshId {
        MeshId(push(&mut self.model.meshes, mesh))
    }

    pub fn add_material(&mut self, material: Material) -> MaterialId {
        MaterialId(push(&mut self.model.materials, material))
    }

    pub fn add_texture(&mut self, texture: Texture) -> TextureId {
        TextureId(push(&mut self.model.textures, texture))
    }

    pub fn add_image(&mut self, image: Image) -> ImageId {
        ImageId(push(&mut self.model.images, image))
    }

    pub fn add_skin(&mut self, skin: Skin) -> SkinId {
        SkinId(push(&mut self.model.skins, skin))
    }

    pub fn add_animation(&mut self, animation: Animation) -> AnimationId {
        AnimationId(push(&mut self.model.animations, animation))
    }

    pub fn add_camera(&mut self, camera: Camera) -> CameraId {
        CameraId(push(&mut self.model.cameras, camera))
    }

    pub fn add_light(&mut self, light: Light) -> LightId {
        LightId(push(&mut self.model.lights, light))
    }

    pub fn warn(&mut self, warning: Warning) {
        self.model.warn(warning);
    }

    pub fn extend_with<E: Extension>(&mut self, extension: E) {
        self.model.extensions.insert(extension);
    }

    pub fn model_mut(&mut self) -> &mut Model {
        &mut self.model
    }

    pub fn finish(mut self) -> Result<Model> {
        self.model.validate()?;
        for mesh in &mut self.model.meshes {
            mesh.finalize();
        }
        Ok(self.model)
    }
}

fn push<T>(items: &mut Vec<T>, value: T) -> u32 {
    let index = items.len() as u32;
    items.push(value);
    index
}
