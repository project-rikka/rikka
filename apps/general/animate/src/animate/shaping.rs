use animate_inspector::EmitterNaming;
use std::cell::RefCell;

use animate_fluid::{
    Aim, Anchor, Emitter, EmitterKey, EmitterSetting, EmitterValue, Preset, Property, Shape, Spot,
};
use animate_inspector::EmitterShaping;
use rikka_model::glam::Vec3;

use crate::animate::Animate;

impl Animate {
    pub(crate) fn attach(&mut self, setting: EmitterSetting, anchor: Anchor, offset: Vec3) {
        let Some(emitter) = self.chosen_emitter().cloned() else {
            return;
        };
        let keys = self.reanchored(&emitter, setting, &anchor);
        self.stage
            .lock()
            .drafts_mut()
            .pour
            .clear(emitter.id, setting);
        self.update_emitter(|emitter| {
            if let Some(spot) = emitter.spot_mut(setting) {
                *spot = Spot { anchor, offset };
            }
            for key in keys {
                emitter.tracks_mut().insert(key);
            }
        });
    }

    fn reanchored(
        &self,
        emitter: &Emitter,
        setting: EmitterSetting,
        to: &Anchor,
    ) -> Vec<EmitterKey> {
        let Some(from) = emitter.spot(setting).map(|spot| &spot.anchor) else {
            return Vec::new();
        };
        if from == to {
            return Vec::new();
        }
        let mut locator = self.stage.lock().locator();
        emitter
            .tracks()
            .keys(&setting)
            .iter()
            .filter_map(|key| {
                let offset = Vec3::from_array(key.value.three()?);
                let world = from.world(offset, |figure, bone| {
                    locator.locate(figure, bone, key.frame)
                })?;
                let moved = to.offset(world, |figure, bone| {
                    locator.locate(figure, bone, key.frame)
                })?;
                Some(EmitterKey {
                    value: EmitterValue::Three(moved.to_array()),
                    ..*key
                })
            })
            .collect()
    }
}

impl EmitterShaping for Animate {
    fn anchor(&mut self, setting: EmitterSetting, anchor: Anchor) {
        let Some(emitter) = self.chosen_emitter().cloned() else {
            return;
        };
        let state = self.pouring_now(&emitter);
        let Some(spot) = state.spot(setting) else {
            return;
        };
        let frame = self.transport.frame();
        let locator = RefCell::new(self.stage.lock().locator());
        let locate = |figure: &str, bone: &str| locator.borrow_mut().locate(figure, bone, frame);
        let offset = spot
            .world(locate)
            .and_then(|world| anchor.offset(world, locate))
            .unwrap_or(Vec3::ZERO);
        self.attach(setting, anchor, offset);
    }
    fn apply_blob(&mut self, size: f32) {
        self.controls.blob_draft = None;
        self.update_emitter(|emitter| {
            if let Shape::Blob { size: current } = &mut emitter.shape {
                *current = size.clamp(*Shape::BLOB_SIZES.start(), *Shape::BLOB_SIZES.end());
            }
        });
    }
    fn apply_property(&mut self, property: Property, value: EmitterValue) {
        self.controls.liquid_draft = None;
        self.update_emitter(|emitter| emitter.liquid.set(property, value));
    }
    fn choose_aim(&mut self, aim: Aim) {
        self.update_emitter(|emitter| emitter.aim = aim);
    }
    fn choose_preset(&mut self, preset: Preset) {
        self.update_emitter(|emitter| emitter.liquid.choose(preset));
    }
    fn choose_shape(&mut self, shape: Shape) {
        self.update_emitter(|emitter| emitter.shape = shape);
    }
    fn choose_surface_figure(&mut self, figure: String) {
        self.update_emitter(|emitter| emitter.shape = Shape::surface(figure, []));
    }
    fn cover_material(&mut self, material: u32, covered: bool) {
        self.update_emitter(|emitter| {
            let Shape::Surface { figure, materials } = &emitter.shape else {
                return;
            };
            let mut chosen = materials.clone();
            chosen.retain(|other| *other != material);
            if covered {
                chosen.push(material);
            }
            emitter.shape = Shape::surface(figure.clone(), chosen);
        });
    }
    fn draft_blob(&mut self, size: f32) {
        self.controls.blob_draft = Some(size);
    }
    fn draft_property(&mut self, property: Property, value: EmitterValue) {
        self.controls.liquid_draft = Some((property, value));
    }
    fn mixed(&self, emitter: &Emitter, property: Property) -> EmitterValue {
        match self.controls.liquid_draft {
            Some((drafted, value)) if drafted == property => value,
            _ => property.read(&emitter.liquid.liquid()),
        }
    }
    fn reset_property(&mut self, property: Property) {
        self.update_emitter(|emitter| emitter.liquid.reset(property));
    }
}
