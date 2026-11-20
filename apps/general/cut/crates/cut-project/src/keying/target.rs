use super::KeyRef;
use crate::animation::{Interpolation, Tracked, Value};
use crate::clip::Clip;
use crate::error::{Error, Result};
use crate::property::{Field, Property};

impl Clip {
    #[must_use]
    pub fn holds(&self, key: &KeyRef) -> bool {
        self.channel(&key.property)
            .is_some_and(|channel| channel.has_key(key.at - self.start))
    }

    #[must_use]
    pub fn interpolation(&self, key: &KeyRef) -> Option<Interpolation> {
        let timing = self.channel(&key.property)?.timing(key.at - self.start)?;
        Some(timing.interpolation())
    }

    pub(crate) fn accepts(&self, property: &Property) -> bool {
        match property {
            Property::Effect { slot, knob } => self
                .effects
                .get(*slot)
                .is_some_and(|effect_slot| effect_slot.effect.value(knob).is_some()),
            _ => Field::from_property(property).is_some_and(|field| self.base(field).is_some()),
        }
    }

    pub(crate) fn keyed<R>(
        &mut self,
        property: &Property,
        change: impl FnOnce(&mut Tracked, &mut Value) -> Result<R>,
    ) -> Result<R> {
        match property {
            Property::Effect { slot, knob } => {
                let slot = self.effects.get_mut(*slot).ok_or(Error::UnknownEffect)?;
                let unknown = || Error::UnknownKnob(knob.clone());
                let knob_spec = slot
                    .effect
                    .knobs()
                    .iter()
                    .find(|candidate| candidate.key == knob)
                    .ok_or_else(unknown)?;
                let before = slot.effect.value(knob).ok_or_else(unknown)?;
                let mut base = Value::Scalar(before);
                let mut tracked = Tracked::Scalar(slot.tracks.remove(knob).unwrap_or_default());
                let result = change(&mut tracked, &mut base);
                tracked.map(|value| match value {
                    Value::Scalar(value) => Value::Scalar(knob_spec.scale.clamp(value)),
                    other => other,
                });
                if let Tracked::Scalar(track) = tracked
                    && !track.is_empty()
                {
                    slot.tracks.insert(knob.clone(), track);
                }
                if let Value::Scalar(value) = base
                    && value != before
                {
                    slot.effect.set(knob, knob_spec.scale.clamp(value));
                }
                result
            }
            _ => {
                let field = Field::from_property(property).ok_or(Error::NotAnimatable)?;
                let before = self.base(field).ok_or(Error::NotAnimatable)?;
                let mut base = before;
                let mut tracked = self
                    .motion
                    .take(field)
                    .unwrap_or_else(|| Tracked::empty_like(before, field.is_spatial()));
                let result = change(&mut tracked, &mut base);
                tracked.map(|value| field.clamp_value(value));
                self.motion.insert(field, tracked);
                if base != before {
                    self.rebase(field, base);
                }
                result
            }
        }
    }
}
