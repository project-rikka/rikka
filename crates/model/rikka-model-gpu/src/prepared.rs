use std::sync::Arc;

use rikka_model_core::Model;
use rikka_model_core::glam::Vec3;
use rikka_renderer::Uploads;
use rustc_hash::FxHashSet;

use crate::materials;
use crate::options::UploadOptions;
use crate::textures::{Picture, Pictures};

#[derive(Debug)]
pub struct Prepared {
    pub(crate) model: Arc<Model>,
    pub(crate) options: UploadOptions,
    pub(crate) pictures: Pictures,
    pub(crate) outline_normals: Vec<Option<Vec<Vec3>>>,
}

impl Prepared {
    #[must_use]
    pub fn new(model: Arc<Model>, options: &UploadOptions) -> Self {
        let mut wanted: Vec<Picture> = Vec::new();
        let mut seen: FxHashSet<Picture> = FxHashSet::default();
        for material in &model.materials {
            materials::bind(&model, material, &mut |picture| {
                if seen.insert(picture) {
                    wanted.push(picture);
                }
                None
            });
        }
        let pictures = Pictures::prepare(&model, &wanted, options);

        let outlined: Vec<bool> = model.materials.iter().map(materials::outlined).collect();
        let outline_normals = model
            .meshes
            .iter()
            .map(|mesh| {
                mesh.submeshes
                    .iter()
                    .filter_map(|submesh| submesh.material)
                    .any(|material| outlined.get(material.index()).copied().unwrap_or(false))
                    .then(|| mesh.smoothed_normals())
            })
            .collect();
        Self {
            model,
            options: *options,
            pictures,
            outline_normals,
        }
    }

    #[must_use]
    pub fn with_uploads(
        model: Arc<Model>,
        options: &UploadOptions,
        uploads: Option<&Uploads>,
    ) -> Self {
        let mut prepared = Self::new(model, options);
        if let Some(uploads) = uploads {
            prepared.create_textures(uploads);
        }
        prepared
    }

    pub fn create_textures(&mut self, uploads: &Uploads) {
        self.pictures.create(uploads);
    }

    #[must_use]
    pub fn model(&self) -> &Arc<Model> {
        &self.model
    }
}
