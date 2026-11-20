use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rikka_model_core::glam::Vec3;
use rikka_model_core::{Instance, MaterialId, MeshId, Model, NodeId, ToonSurface};
use rikka_renderer::{Reclaimer, Renderer};
use rikka_renderer_3d::Three;
use rikka_renderer_3d::{MaterialDesc, MaterialId as GpuMaterial, MeshId as GpuMesh, Reclaim3d};

use crate::error::{Error, Result};
use crate::materials;
use crate::meshes;
use crate::options::UploadOptions;
use crate::prepared::Prepared;
use crate::styles::Base;
use crate::textures::Textures;

#[derive(Debug)]
pub struct GpuModel {
    model: Arc<Model>,
    meshes: Vec<GpuMesh>,
    materials: Vec<GpuMaterial>,
    outlines: Vec<Option<GpuMaterial>>,
    fallback: GpuMaterial,
    textures: Textures,
    reclaimer: Reclaimer,
    wet_offsets: Vec<u32>,
    painted: AtomicU64,
    bases: Vec<Base>,
}

impl GpuModel {
    pub fn upload(renderer: &mut Renderer, model: Arc<Model>) -> Result<Self> {
        Self::upload_with(renderer, model, &UploadOptions::default())
    }

    pub fn upload_with(
        renderer: &mut Renderer,
        model: Arc<Model>,
        options: &UploadOptions,
    ) -> Result<Self> {
        Self::upload_prepared(renderer, Prepared::new(model, options))
    }

    pub fn upload_prepared(renderer: &mut Renderer, prepared: Prepared) -> Result<Self> {
        let Prepared {
            model,
            options,
            pictures,
            outline_normals,
        } = prepared;
        let fallback = materials::fallback()
            .and_then(|fallback| renderer.three().create_material(fallback))
            .map_err(Error::material)?;
        let mut built = Self {
            wet_offsets: wet_offsets(&model),
            model,
            meshes: Vec::new(),
            materials: Vec::new(),
            outlines: Vec::new(),
            fallback,
            textures: Textures::upload(renderer, pictures),
            reclaimer: renderer.reclaimer(),
            painted: AtomicU64::new(0),
            bases: Vec::new(),
        };
        match built.fill(renderer, &options, outline_normals) {
            Ok(()) => Ok(built),
            Err(error) => {
                built.destroy(renderer);
                Err(error)
            }
        }
    }

    pub fn destroy(self, renderer: &mut Renderer) {
        drop(self);
        renderer.reclaim();
    }

    #[must_use]
    pub fn model(&self) -> &Arc<Model> {
        &self.model
    }

    #[must_use]
    pub fn instance(&self) -> Instance {
        Instance::new(Arc::clone(&self.model))
    }

    #[must_use]
    pub fn mesh(&self, mesh: MeshId) -> Option<GpuMesh> {
        self.meshes.get(mesh.index()).copied()
    }

    #[must_use]
    pub fn material(&self, material: Option<MaterialId>) -> GpuMaterial {
        material
            .and_then(|id| self.materials.get(id.index()).copied())
            .unwrap_or(self.fallback)
    }

    #[must_use]
    pub fn wet_vertices(&self) -> u32 {
        self.wet_offsets.last().copied().unwrap_or(0)
    }

    #[must_use]
    pub fn wet_offset(&self, node: NodeId) -> Option<u32> {
        self.model.nodes.get(node.index())?.mesh?;
        self.wet_offsets.get(node.index()).copied()
    }

    #[must_use]
    pub fn outline(&self, material: Option<MaterialId>) -> Option<GpuMaterial> {
        self.outlines.get(material?.index()).copied().flatten()
    }

    #[must_use]
    pub fn is_outlined(&self, material: MaterialId) -> bool {
        self.outline(Some(material)).is_some()
    }

    #[must_use]
    pub fn preset(&self, material: MaterialId) -> Option<&str> {
        let name = self.bases.get(material.index())?.styled.template().name();
        rikka_shader_presets_gpu::keys()
            .any(|key| key == name)
            .then_some(name)
    }

    #[must_use]
    pub fn class(&self, material: MaterialId) -> Option<ToonSurface> {
        materials::class(self.model.materials.get(material.index())?)
    }

    pub(crate) fn reclaimer(&self) -> Reclaimer {
        self.reclaimer.clone()
    }

    pub(crate) fn bases(&self) -> &[Base] {
        &self.bases
    }

    pub(crate) fn replace(
        &mut self,
        at: usize,
        main: GpuMaterial,
        outline: Option<GpuMaterial>,
        styled: MaterialDesc,
    ) {
        let old = std::mem::replace(&mut self.materials[at], main);
        self.reclaimer.material(old);
        if let Some(old) = std::mem::replace(&mut self.outlines[at], outline) {
            self.reclaimer.material(old);
        }
        self.bases[at].styled = styled;
    }

    pub fn refresh_materials(&self, renderer: &mut Renderer, instance: &Instance) -> Result<()> {
        let Some(tints) = instance.tints() else {
            return Ok(());
        };
        if self.painted.load(Ordering::Relaxed) == tints.revision() {
            return Ok(());
        }
        for (id, material) in tints.iter() {
            let mut values = materials::values(material, self.has_occlusion(id));
            if let Some(base) = self.bases.get(id.index()) {
                values.retain(|(name, value)| {
                    base.styled.value(name).is_some_and(|styled| {
                        std::mem::discriminant(styled) == std::mem::discriminant(value)
                    })
                });
            }
            for gpu in [
                self.materials.get(id.index()),
                self.outlines.get(id.index()).copied().flatten().as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                renderer
                    .three()
                    .set_material_values(*gpu, &values)
                    .map_err(Error::material)?;
            }
        }
        self.painted.store(tints.revision(), Ordering::Relaxed);
        Ok(())
    }

    fn fill(
        &mut self,
        renderer: &mut Renderer,
        options: &UploadOptions,
        outline_normals: Vec<Option<Vec<Vec3>>>,
    ) -> Result<()> {
        let model = Arc::clone(&self.model);
        for material in &model.materials {
            let bindings =
                materials::bind(&model, material, &mut |picture| self.textures.get(picture));
            let desc =
                materials::describe(material, &bindings, |sampler| options.filtering(sampler))
                    .map_err(Error::material)?;
            let created = renderer.three().create_material(desc.clone());
            self.materials.push(created.map_err(Error::material)?);

            let outline = match materials::outlined(material) {
                false => None,
                true => Some(
                    materials::describe_outline(material, &bindings, |sampler| {
                        options.filtering(sampler)
                    })
                    .map_err(Error::material)?,
                ),
            };
            self.outlines.push(match &outline {
                Some(outline) => Some(
                    renderer
                        .three()
                        .create_material(outline.clone())
                        .map_err(Error::material)?,
                ),
                None => None,
            });
            self.bases.push(Base::new(desc, outline));
        }
        for (mesh, outline_normals) in model.meshes.iter().zip(outline_normals) {
            let uploaded =
                meshes::upload(renderer, mesh, outline_normals.as_deref().unwrap_or(&[]))?;
            self.meshes.push(uploaded);
        }
        Ok(())
    }

    fn has_occlusion(&self, material: MaterialId) -> bool {
        self.model
            .materials
            .get(material.index())
            .and_then(|material| {
                material
                    .textures
                    .metallic_roughness
                    .or(material.textures.occlusion)
            })
            .is_some()
    }
}

fn wet_offsets(model: &Model) -> Vec<u32> {
    let mut total = 0u32;
    let mut offsets = Vec::with_capacity(model.nodes.len() + 1);
    for node in &model.nodes {
        offsets.push(total);
        if let Some(mesh) = node.mesh {
            total = total.saturating_add(model.mesh(mesh).vertices.positions.len() as u32);
        }
    }
    offsets.push(total);
    offsets
}

impl Drop for GpuModel {
    fn drop(&mut self) {
        for mesh in self.meshes.drain(..) {
            self.reclaimer.mesh(mesh);
        }
        for texture in self.textures.take_uploaded() {
            self.reclaimer.texture(texture);
        }
        let materials = self
            .materials
            .drain(..)
            .chain(self.outlines.drain(..).flatten())
            .chain(std::iter::once(self.fallback));
        for material in materials {
            self.reclaimer.material(material);
        }
    }
}
