use rikka_model_core::MaterialId;
use rikka_renderer::Renderer;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::{MaterialDesc, MaterialTemplate, Value};

use crate::error::{Error, Result};
use crate::gpu_model::GpuModel;
use crate::materials;

#[derive(Debug)]
pub(crate) struct Base {
    pub(crate) original: MaterialDesc,
    pub(crate) outline: Option<MaterialDesc>,
    pub(crate) styled: MaterialDesc,
}

impl Base {
    pub(crate) fn new(original: MaterialDesc, outline: Option<MaterialDesc>) -> Self {
        Self {
            styled: original.clone(),
            original,
            outline,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MaterialStyle<'a> {
    pub template: Option<&'a MaterialTemplate>,
    pub preset: Option<&'a str>,
    pub values: &'a [(&'a str, Value)],
    pub outline: Option<bool>,
}

impl GpuModel {
    #[must_use]
    pub fn base(&self, material: MaterialId) -> Option<&MaterialDesc> {
        self.bases().get(material.index()).map(|base| &base.styled)
    }

    pub fn apply_style(
        &mut self,
        renderer: &mut Renderer,
        material: MaterialId,
        style: MaterialStyle<'_>,
    ) -> Result<()> {
        let at = material.index();
        let base = self.bases().get(at).ok_or(Error::NoMaterial(at))?;
        let styled = styled(&base.original, style).map_err(Error::material)?;
        let drawn = with_values(styled.clone(), style.values).map_err(Error::material)?;
        let outline = match (style.outline, &base.outline) {
            (Some(false), _) | (None, None) => None,
            (Some(true), None) => Some(with_values(
                styled
                    .clone()
                    .state(materials::outline_state(self.model().material(material)))
                    .hull(),
                style.values,
            )),
            (_, Some(outline)) => {
                Some(self::styled(outline, style).and_then(|desc| with_values(desc, style.values)))
            }
        }
        .transpose()
        .map_err(Error::material)?;
        let main = renderer
            .three()
            .create_material(drawn)
            .map_err(Error::material)?;
        let outline = match outline {
            Some(desc) => match renderer.three().create_material(desc) {
                Ok(id) => Some(id),
                Err(error) => {
                    renderer
                        .three()
                        .destroy_material(main)
                        .map_err(Error::material)?;
                    return Err(Error::material(error));
                }
            },
            None => None,
        };
        self.replace(at, main, outline, styled);
        Ok(())
    }

    pub fn set_values(
        &self,
        renderer: &mut Renderer,
        material: MaterialId,
        values: &[(&str, Value)],
    ) -> Result<()> {
        let at = material.index();
        if at >= self.bases().len() {
            return Err(Error::NoMaterial(at));
        }
        renderer
            .three()
            .set_material_values(self.material(Some(material)), values)
            .map_err(Error::material)?;
        if let Some(outline) = self.outline(Some(material)) {
            renderer
                .three()
                .set_material_values(outline, values)
                .map_err(Error::material)?;
        }
        Ok(())
    }
}

type Described = rikka_renderer_3d::Result<MaterialDesc>;

fn styled(desc: &MaterialDesc, style: MaterialStyle<'_>) -> Described {
    let desc = match style.template {
        Some(template) if template != desc.template() => desc.to_template(template),
        _ => desc.clone(),
    };
    Ok(match style.preset {
        Some(preset) => desc.preset(preset)?,
        None => desc,
    })
}

fn with_values(mut desc: MaterialDesc, values: &[(&str, Value)]) -> Described {
    for (name, value) in values {
        desc.set(name, value.clone())?;
    }
    Ok(desc)
}
