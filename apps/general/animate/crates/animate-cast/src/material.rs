use rikka_model::{Model, Shading};

pub(crate) fn material_class(model: &Model, material: u32) -> Option<&'static str> {
    match model.materials.get(material as usize)?.shading {
        Shading::Toon(toon) => Some(rikka_model_gpu::class_preset(toon.surface).preset()),
        Shading::Pbr(_) => None,
    }
}
