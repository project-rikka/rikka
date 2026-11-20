use rikka_model_core::MaterialId;
use rikka_renderer::{Reclaimer, Renderer};
use rikka_renderer_3d::Three;
use rikka_renderer_3d::{MaterialId as GpuMaterial, MaterialTemplate, Reclaim3d};

use crate::error::{Error, Result};
use crate::gpu_model::GpuModel;

#[derive(Debug)]
pub struct Restyle {
    template: MaterialTemplate,
    materials: Vec<Option<GpuMaterial>>,
    reclaimer: Reclaimer,
}

impl Restyle {
    #[must_use]
    pub fn template(&self) -> &MaterialTemplate {
        &self.template
    }

    pub(crate) fn material(&self, material: Option<MaterialId>) -> Option<GpuMaterial> {
        self.materials.get(material?.index()).copied().flatten()
    }
}

impl Drop for Restyle {
    fn drop(&mut self) {
        for material in self.materials.drain(..).flatten() {
            self.reclaimer.material(material);
        }
    }
}
impl GpuModel {
    pub fn restyle(&self, renderer: &mut Renderer, template: &MaterialTemplate) -> Result<Restyle> {
        self.restyle_where(renderer, template, |_| true)
    }

    pub fn restyle_material(
        &self,
        renderer: &mut Renderer,
        template: &MaterialTemplate,
        material: MaterialId,
    ) -> Result<Restyle> {
        self.restyle_where(renderer, template, |index| index == material.index())
    }

    fn restyle_where(
        &self,
        renderer: &mut Renderer,
        template: &MaterialTemplate,
        chosen: impl Fn(usize) -> bool,
    ) -> Result<Restyle> {
        let mut restyle = Restyle {
            template: template.clone(),
            materials: Vec::with_capacity(self.model().materials.len()),
            reclaimer: self.reclaimer(),
        };
        for index in 0..self.model().materials.len() {
            let material = match chosen(index) {
                true => {
                    let desc = self.bases()[index].original.to_template(template);
                    let created = renderer.three().create_material(desc);
                    Some(created.map_err(Error::material)?)
                }
                false => None,
            };
            restyle.materials.push(material);
        }
        Ok(restyle)
    }
}
