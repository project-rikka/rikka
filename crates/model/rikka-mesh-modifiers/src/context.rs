use std::any::Any;
use std::sync::Arc;

use glam::{Mat4, Vec3, Vec4};
use rikka_polymesh::Polymesh;

use crate::reference::{CollectionRef, MaterialRef, ObjectRef, TextureRef};
use crate::target::Target;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureSample {
    pub intensity: f32,
    pub color: Vec4,
}

pub trait Texture: Send + Sync {
    fn sample(&self, coordinate: Vec3) -> TextureSample;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bone {
    pub name: String,
    pub selected: bool,
    pub deform: bool,
}

pub trait Context {
    fn object_matrix(&self) -> Mat4;

    fn original_positions(&self) -> Option<&[Vec3]> {
        None
    }

    fn material_count(&self) -> usize;

    fn object(&self) -> Option<ObjectRef> {
        None
    }

    fn matrix(&self, _object: ObjectRef) -> Option<Mat4> {
        None
    }

    fn mesh(&self, _object: ObjectRef) -> Option<Arc<Polymesh>> {
        None
    }

    fn target(&self, object: ObjectRef) -> Option<Arc<Target>> {
        self.mesh(object)
            .and_then(|mesh| Target::new(mesh).ok())
            .map(Arc::new)
    }

    fn texture(&self, _texture: TextureRef) -> Option<&dyn Texture> {
        None
    }

    fn bone_matrix(&self, _armature: ObjectRef, _bone: &str) -> Option<Mat4> {
        None
    }

    fn deforming_bones(&self, _object: ObjectRef) -> Option<Vec<Bone>> {
        None
    }

    fn armature(&self, _object: ObjectRef) -> Option<&dyn Any> {
        None
    }

    fn materials(&self) -> Vec<Option<MaterialRef>> {
        vec![None; self.material_count()]
    }

    fn object_materials(&self, _object: ObjectRef) -> Option<Vec<Option<MaterialRef>>> {
        None
    }

    fn collection_objects(&self, _collection: CollectionRef) -> Option<Vec<ObjectRef>> {
        None
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Standalone {
    pub matrix: Mat4,
    pub material_count: usize,
}

impl Context for Standalone {
    fn object_matrix(&self) -> Mat4 {
        self.matrix
    }

    fn material_count(&self) -> usize {
        self.material_count
    }
}
