use std::sync::atomic::{AtomicU64, Ordering};

use crate::material::{Material, MaterialId};
use crate::model::Model;

static REVISIONS: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Default)]
pub struct Tints {
    materials: Vec<(MaterialId, Material)>,
    revision: u64,
}

impl Tints {
    #[must_use]
    pub fn for_materials(model: &Model, materials: impl IntoIterator<Item = MaterialId>) -> Self {
        let mut ids: Vec<MaterialId> = materials
            .into_iter()
            .filter(|id| id.index() < model.materials.len())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        Self {
            materials: ids
                .into_iter()
                .map(|id| (id, model.material(id).clone()))
                .collect(),
            revision: next_revision(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.materials.is_empty()
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn get(&self, id: MaterialId) -> Option<&Material> {
        let position = self
            .materials
            .binary_search_by_key(&id, |(materials, _)| *materials)
            .ok()?;
        Some(&self.materials[position].1)
    }

    pub fn get_mut(&mut self, id: MaterialId) -> Option<&mut Material> {
        let position = self
            .materials
            .binary_search_by_key(&id, |(materials, _)| *materials)
            .ok()?;
        self.revision = next_revision();
        Some(&mut self.materials[position].1)
    }

    pub fn iter(&self) -> impl Iterator<Item = (MaterialId, &Material)> {
        self.materials.iter().map(|(id, material)| (*id, material))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (MaterialId, &mut Material)> {
        self.revision = next_revision();
        self.materials
            .iter_mut()
            .map(|(id, material)| (*id, material))
    }

    pub fn restore(&mut self, model: &Model) {
        self.revision = next_revision();
        for (id, material) in &mut self.materials {
            material.clone_from(model.material(*id));
        }
    }
}

fn next_revision() -> u64 {
    REVISIONS.fetch_add(1, Ordering::Relaxed)
}
