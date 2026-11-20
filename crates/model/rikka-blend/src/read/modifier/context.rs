use std::cell::RefCell;
use std::sync::Arc;

use rikka_mesh_modifiers::{CollectionRef, Context, ObjectRef, TextureRef};
use rikka_model_core::glam::Mat4;
use rikka_polymesh::Polymesh;

use super::{References, Scope};
use crate::file::Record;
use crate::read::mesh::source;
use crate::read::scene::OB_MESH;

pub(super) struct Objects<'a> {
    records: RefCell<Vec<Record<'a>>>,
}

impl<'a> Objects<'a> {
    pub(super) fn new(object: Record<'a>) -> Objects<'a> {
        Objects {
            records: RefCell::new(vec![object]),
        }
    }

    fn get(&self, object: ObjectRef) -> Option<Record<'a>> {
        self.records.borrow().get(object.get() as usize).copied()
    }
}

impl<'a> References<'a> for Objects<'a> {
    fn object(&self, record: Option<Record<'a>>) -> Option<ObjectRef> {
        let record = record?;
        let mut records = self.records.borrow_mut();
        let index = match records
            .iter()
            .position(|known| known.identity() == record.identity())
        {
            Some(index) => index,
            None => {
                records.push(record);
                records.len() - 1
            }
        };
        Some(ObjectRef::new(index as u64))
    }

    fn texture(&self, record: Option<Record<'a>>) -> Option<TextureRef> {
        record.map(|_| TextureRef::new(0))
    }

    fn collection(&self, _record: Option<Record<'a>>) -> Option<CollectionRef> {
        None
    }
}

pub(super) struct BlendContext<'r, 's, 'a> {
    pub(super) scope: &'r Scope<'s, 'a>,
    pub(super) objects: &'r Objects<'a>,
    pub(super) material_count: usize,
}

impl Context for BlendContext<'_, '_, '_> {
    fn object_matrix(&self) -> Mat4 {
        self.scope.world
    }

    fn material_count(&self) -> usize {
        self.material_count
    }

    fn object(&self) -> Option<ObjectRef> {
        Some(ObjectRef::new(0))
    }

    fn matrix(&self, object: ObjectRef) -> Option<Mat4> {
        self.objects
            .get(object)
            .map(|record| (self.scope.world_of)(record))
    }

    fn mesh(&self, object: ObjectRef) -> Option<Arc<Polymesh>> {
        let record = self.objects.get(object)?;
        if record.int("type") != Some(OB_MESH) {
            return None;
        }
        let data = record.id("data")?;
        Some(Arc::new(source::read(data, Some(record)).mesh))
    }
}
