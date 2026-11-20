use glam::Mat4;

use crate::animation::{Animation, AnimationId};
use crate::bounds::Bounds;
use crate::camera::{Camera, CameraId};
use crate::error::{Error, Result};
use crate::extension::Extensions;
use crate::light::{Light, LightId};
use crate::material::{Material, MaterialAlias, MaterialId};
use crate::mesh::{Mesh, MeshId};
use crate::metadata::Metadata;
use crate::node::{Node, NodeId};
use crate::rig_facts::RigFacts;
use crate::skin::{Skin, SkinId};
use crate::texture::{Image, ImageId, Texture, TextureId};
use crate::transform::Transform;
use crate::warning::Warning;

#[derive(Clone, Debug, Default)]
pub struct Model {
    pub nodes: Vec<Node>,
    pub roots: Vec<NodeId>,
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    pub textures: Vec<Texture>,
    pub images: Vec<Image>,
    pub skins: Vec<Skin>,
    pub animations: Vec<Animation>,
    pub cameras: Vec<Camera>,
    pub lights: Vec<Light>,
    pub metadata: Metadata,
    pub warnings: Vec<Warning>,
    pub extensions: Extensions,
}

impl Model {
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.index()]
    }

    #[must_use]
    pub fn mesh(&self, id: MeshId) -> &Mesh {
        &self.meshes[id.index()]
    }

    pub fn material(&self, id: MaterialId) -> &Material {
        &self.materials[id.index()]
    }

    #[must_use]
    pub fn texture(&self, id: TextureId) -> &Texture {
        &self.textures[id.index()]
    }

    #[must_use]
    pub fn image(&self, id: ImageId) -> &Image {
        &self.images[id.index()]
    }

    #[must_use]
    pub fn skin(&self, id: SkinId) -> &Skin {
        &self.skins[id.index()]
    }

    #[must_use]
    pub fn animation(&self, id: AnimationId) -> &Animation {
        &self.animations[id.index()]
    }

    #[must_use]
    pub fn camera(&self, id: CameraId) -> &Camera {
        &self.cameras[id.index()]
    }

    #[must_use]
    pub fn light(&self, id: LightId) -> &Light {
        &self.lights[id.index()]
    }

    #[must_use]
    pub fn find_node(&self, name: &str) -> Option<NodeId> {
        self.nodes
            .iter()
            .position(|node| node.name == name)
            .map(|index| NodeId(index as u32))
    }

    #[must_use]
    pub fn animation_id(&self, index: usize) -> Option<AnimationId> {
        (index < self.animations.len()).then_some(AnimationId(index as u32))
    }

    #[must_use]
    pub fn find_animation(&self, name: &str) -> Option<AnimationId> {
        self.animations
            .iter()
            .position(|animation| animation.name == name)
            .map(|index| AnimationId(index as u32))
    }

    pub fn visit(&self, mut each: impl FnMut(NodeId)) {
        let mut visited = vec![false; self.nodes.len()];
        let mut stack: Vec<NodeId> = self.roots.iter().rev().copied().collect();
        while let Some(id) = stack.pop() {
            let Some(seen) = visited.get_mut(id.index()) else {
                continue;
            };
            if std::mem::replace(seen, true) {
                continue;
            }
            each(id);
            stack.extend(self.node(id).children.iter().rev().copied());
        }
    }

    #[must_use]
    pub fn order(&self) -> Vec<NodeId> {
        let mut order = Vec::with_capacity(self.nodes.len());
        self.visit(|node| order.push(node));
        order
    }

    #[must_use]
    pub fn world_transforms(&self) -> Vec<Mat4> {
        let locals: Vec<Transform> = self.nodes.iter().map(|node| node.transform).collect();
        let mut world = vec![Mat4::IDENTITY; self.nodes.len()];
        self.place_all(&self.order(), &locals, &mut world);
        world
    }

    #[must_use]
    pub fn parent_world(&self, node: NodeId, world: &[Mat4]) -> Mat4 {
        self.node(node)
            .parent
            .map_or(Mat4::IDENTITY, |parent| world[parent.index()])
    }

    pub fn place(&self, node: NodeId, locals: &[Transform], world: &mut [Mat4]) {
        world[node.index()] = self.parent_world(node, world) * locals[node.index()].to_matrix();
    }

    pub fn place_all(&self, order: &[NodeId], locals: &[Transform], world: &mut [Mat4]) {
        for &node in order {
            self.place(node, locals, world);
        }
    }

    #[must_use]
    pub fn bounds(&self) -> Bounds {
        let world = self.world_transforms();
        bounds_of(self, |node| world[node.index()])
    }

    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.meshes.iter().map(|mesh| mesh.vertices.len()).sum()
    }

    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.meshes.iter().map(Mesh::triangle_count).sum()
    }

    #[must_use]
    pub fn rig_facts(&self) -> RigFacts {
        let mut facts = RigFacts::default();
        self.extensions.describe_rig(&mut facts);
        facts.retain_nodes(self.nodes.len());
        facts
    }

    #[must_use]
    pub fn material_aliases(&self) -> Vec<MaterialAlias> {
        let mut aliases = Vec::new();
        self.extensions.alias_materials(&mut aliases);
        aliases.retain(|alias| alias.material.index() < self.materials.len());
        aliases
    }

    pub fn warn(&mut self, warning: Warning) {
        tracing::debug!(model = ?self.metadata.source, %warning, "model import warning");
        self.warnings.push(warning);
    }

    pub fn validate(&self) -> Result<()> {
        for mesh in &self.meshes {
            mesh.validate(self.materials.len())?;
        }
        for skin in &self.skins {
            skin.validate(self.nodes.len())?;
        }
        for animation in &self.animations {
            animation.validate(self.nodes.len())?;
        }
        for texture in &self.textures {
            if texture.image.index() >= self.images.len() {
                return Err(Error::Invalid(format!(
                    "texture \"{}\" references missing {}",
                    texture.name, texture.image
                )));
            }
        }
        for material in &self.materials {
            for texture in material.textures.each().into_iter().flatten() {
                if texture.index() >= self.textures.len() {
                    return Err(Error::Invalid(format!(
                        "material \"{}\" references missing {texture}",
                        material.name
                    )));
                }
            }
        }
        self.validate_hierarchy()
    }

    fn validate_hierarchy(&self) -> Result<()> {
        let mut listed = vec![false; self.nodes.len()];
        for (index, node) in self.nodes.iter().enumerate() {
            let named = |what: &str, present: bool, id: &dyn std::fmt::Display| match present {
                true => Ok(()),
                false => Err(Error::Invalid(format!(
                    "node \"{}\" references missing {what} {id}",
                    node.name
                ))),
            };
            if let Some(mesh) = node.mesh {
                named("mesh", mesh.index() < self.meshes.len(), &mesh)?;
            }
            if let Some(skin) = node.skin {
                named("skin", skin.index() < self.skins.len(), &skin)?;
            }
            if let Some(camera) = node.camera {
                named("camera", camera.index() < self.cameras.len(), &camera)?;
            }
            if let Some(light) = node.light {
                named("light", light.index() < self.lights.len(), &light)?;
            }
            for child in &node.children {
                let Some(node) = self.nodes.get(child.index()) else {
                    return Err(Error::Invalid(format!(
                        "node {index} references missing child {child}"
                    )));
                };
                if node.parent != Some(NodeId(index as u32)) {
                    return Err(Error::Invalid(format!(
                        "child {child} of node {index} does not name it as parent"
                    )));
                }
                listed[child.index()] = true;
            }
        }
        for (index, node) in self.nodes.iter().enumerate() {
            if node.parent.is_some() && !listed[index] {
                return Err(Error::Invalid(format!(
                    "node {index} names a parent that does not list it as child"
                )));
            }
        }
        for root in &self.roots {
            if self
                .nodes
                .get(root.index())
                .is_none_or(|node| node.parent.is_some())
            {
                return Err(Error::Invalid(format!(
                    "root {root} is missing or has a parent"
                )));
            }
        }

        let mut reached = 0;
        self.visit(|_| reached += 1);
        match reached == self.nodes.len() {
            true => Ok(()),
            false => Err(Error::Invalid(format!(
                "only {reached} of {} nodes are reachable from the roots: hierarchy is disconnected or cyclic",
                self.nodes.len()
            ))),
        }
    }
}

pub(crate) fn bounds_of(model: &Model, placement: impl Fn(NodeId) -> Mat4) -> Bounds {
    model
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| Some((NodeId(index as u32), node.mesh?)))
        .fold(Bounds::EMPTY, |bounds, (node, mesh)| {
            bounds.union(model.mesh(mesh).bounds().transformed(placement(node)))
        })
}
