use rikka_model_core::{Material, Sampler};
use rikka_renderer::TextureId as GpuTexture;
use rikka_renderer_3d::{MaterialDesc, MaterialState};

use crate::error::{Error, Result};
use crate::materials::{self, Binding, MaterialImage};
use crate::options::UploadOptions;

#[derive(Clone, Debug)]
pub struct MaterialDescs {
    pub main: MaterialDesc,
    pub state: MaterialState,
    pub outline: Option<MaterialDesc>,
}

pub fn describe_material(
    material: &Material,
    options: &UploadOptions,
    texture: &mut dyn FnMut(MaterialImage) -> Option<(GpuTexture, Sampler)>,
) -> Result<MaterialDescs> {
    let bindings = materials::bind_with(material, &mut |image| match texture(image) {
        Some((texture, sampler)) => Binding {
            texture: Some(texture),
            sampler,
        },
        None => Binding::default(),
    });
    let filtering = |sampler| options.filtering(sampler);
    let main = materials::describe(material, &bindings, filtering).map_err(Error::material)?;
    let outline = match materials::outlined(material) {
        true => Some(
            materials::describe_outline(material, &bindings, filtering).map_err(Error::material)?,
        ),
        false => None,
    };
    Ok(MaterialDescs {
        main,
        state: materials::state(material),
        outline,
    })
}
