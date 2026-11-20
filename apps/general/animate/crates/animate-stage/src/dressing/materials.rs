use std::collections::BTreeMap;
use std::sync::Arc;

use animate_look::{Evaluated, LookValue, MaterialValue, Name};
use animate_shader::{SharedShaders, Status};
use rikka_model::MaterialId;
use rikka_model_gpu::{GpuModel, MaterialStyle, class_preset};
use rikka_renderer::Renderer;
use rikka_renderer_3d::{MaterialTemplate, TemplateReadiness, Value};
use rikka_shader_compile::Material;

use crate::dressing::assets::Assets;
use crate::dressing::wake::Wake;

#[derive(Debug)]
struct Applied {
    template: MaterialTemplate,
    outlined: bool,
    values: Vec<(String, Value)>,
    failed: bool,
}

#[derive(Clone, Copy)]
struct SlotRef<'a> {
    figure: Name,
    index: u32,
    key: &'a str,
}

#[derive(Debug, Default)]
pub(super) struct Dressed {
    slots: BTreeMap<u32, Applied>,
}

pub(super) struct Sources<'a> {
    pub(super) shaders: &'a SharedShaders,
    pub(super) assets: &'a mut Assets,
    pub(super) wake: &'a Wake,
    pub(super) failures: &'a mut Vec<String>,
}

impl Dressed {
    pub(super) fn dress(
        &mut self,
        renderer: &mut Renderer,
        gpu: &mut GpuModel,
        figure: Name,
        evaluated: &Evaluated<'_>,
        sources: &mut Sources<'_>,
    ) {
        let count = gpu.model().materials.len() as u32;
        let look = evaluated.look();
        let wanted: BTreeMap<u32, String> = look
            .slots
            .figure(figure)
            .filter(|(index, _)| *index < count)
            .filter_map(|(index, slot)| Some((index, slot.material.clone()?)))
            .collect();
        let reverted: Vec<u32> = self
            .slots
            .keys()
            .copied()
            .filter(|index| !wanted.contains_key(index))
            .collect();
        for index in reverted {
            self.slots.remove(&index);
            if let Err(error) =
                gpu.apply_style(renderer, MaterialId(index), MaterialStyle::default())
            {
                sources.failures.push(format!("material {index}: {error}"));
            }
        }
        for (index, key) in wanted {
            let compiled = {
                let shaders = sources.shaders.lock();
                if matches!(shaders.status(&key), Some(Status::Compiling) | None) {
                    sources.wake.wait();
                }
                shaders.material(&key)
            };
            let Some(compiled) = compiled else {
                continue;
            };
            let slot = SlotRef {
                figure,
                index,
                key: &key,
            };
            self.apply(renderer, gpu, slot, &compiled, evaluated, sources);
        }
    }

    fn apply(
        &mut self,
        renderer: &mut Renderer,
        gpu: &mut GpuModel,
        slot: SlotRef<'_>,
        compiled: &Arc<Material>,
        evaluated: &Evaluated<'_>,
        sources: &mut Sources<'_>,
    ) {
        let SlotRef { figure, index, key } = slot;
        let template = compiled.template();
        let outlined = compiled.source().outlined;
        let failure = match template.watch(sources.wake.rouse()) {
            TemplateReadiness::Built => None,
            TemplateReadiness::Failed(error) => Some(error),
            TemplateReadiness::Pending | TemplateReadiness::Drawable => {
                sources.wake.wait();
                None
            }
        };
        let schema = template.schema();
        let material = MaterialId(index);
        let class = gpu
            .class(material)
            .map(|surface| class_preset(surface).preset())
            .filter(|class| schema.presets().contains(class));
        let mut values = Vec::new();
        for (name, value) in evaluated.material_values(figure, index, key, compiled.sockets()) {
            if schema.parameter(&name).is_none() {
                continue;
            }
            let value = match value {
                MaterialValue::Value(value) => value,
                MaterialValue::Texture { path, kind } => {
                    match sources.assets.texture(
                        renderer,
                        &path,
                        kind,
                        sources.wake,
                        sources.failures,
                    ) {
                        Some(binding) => Value::Texture(Some(binding)),
                        None => continue,
                    }
                }
            };
            values.push((name, value));
        }
        let current = self
            .slots
            .get(&index)
            .is_some_and(|applied| applied.template == *template && applied.outlined == outlined);
        let reported = current && self.slots.get(&index).is_some_and(|applied| applied.failed);
        if let Some(error) = &failure
            && !reported
        {
            sources.failures.push(format!("material {index}: {error}"));
        }
        if current {
            let applied = self.slots.get_mut(&index).expect("the slot is applied");
            applied.failed = failure.is_some();
            if applied.values == values {
                return;
            }
            let mut changed: Vec<(&str, Value)> = values
                .iter()
                .filter(|(name, value)| {
                    !applied
                        .values
                        .iter()
                        .any(|(held, before)| held == name && before == value)
                })
                .map(|(name, value)| (name.as_str(), value.clone()))
                .collect();
            for (name, _) in &applied.values {
                if values.iter().any(|(held, _)| held == name) {
                    continue;
                }
                if let Some(spec) = schema.parameter(name) {
                    let resting = class.and_then(|class| spec.preset(class)).map_or_else(
                        || LookValue::default_of(spec),
                        |literal| LookValue::from_literal(spec, literal),
                    );
                    changed.push((name.as_str(), resting.to_setting(|_| None)));
                }
            }
            if let Err(error) = gpu.set_values(renderer, material, &changed) {
                sources.failures.push(format!("material {index}: {error}"));
            }
            applied.values = values;
            return;
        }
        let named: Vec<(&str, Value)> = values
            .iter()
            .map(|(name, value)| (name.as_str(), value.clone()))
            .collect();
        let styled = gpu.apply_style(
            renderer,
            material,
            MaterialStyle {
                template: Some(template),
                preset: class,
                values: &named,
                outline: Some(outlined),
            },
        );
        if let Err(error) = styled {
            sources.failures.push(format!("material {index}: {error}"));
        }
        self.slots.insert(
            index,
            Applied {
                template: template.clone(),
                outlined,
                values,
                failed: failure.is_some(),
            },
        );
    }
}
