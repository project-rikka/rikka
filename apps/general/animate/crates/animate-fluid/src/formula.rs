use std::collections::BTreeMap;

use crate::property::Property;
use crate::value::EmitterValue;
use crate::{Liquid, Preset};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Formula {
    preset: Preset,
    overrides: BTreeMap<Property, EmitterValue>,
}

impl Formula {
    #[must_use]
    pub fn from_preset(preset: Preset) -> Self {
        Self {
            preset,
            overrides: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn preset(&self) -> Preset {
        self.preset
    }

    pub fn overrides(&self) -> impl Iterator<Item = (Property, EmitterValue)> + '_ {
        self.overrides
            .iter()
            .map(|(property, value)| (*property, *value))
    }

    #[must_use]
    pub fn overrides_property(&self, property: Property) -> bool {
        self.overrides.contains_key(&property)
    }

    #[must_use]
    pub fn liquid(&self) -> Liquid {
        let mut liquid = self.preset.liquid();
        for (property, value) in &self.overrides {
            property.write(&mut liquid, *value);
        }
        liquid
    }

    pub fn choose(&mut self, preset: Preset) {
        self.preset = preset;
        self.overrides.clear();
    }

    pub fn set(&mut self, property: Property, value: EmitterValue) {
        match property.read(&self.preset.liquid()) == value {
            true => {
                self.overrides.remove(&property);
            }
            false => {
                self.overrides.insert(property, value);
            }
        }
    }

    pub fn reset(&mut self, property: Property) {
        self.overrides.remove(&property);
    }
}
