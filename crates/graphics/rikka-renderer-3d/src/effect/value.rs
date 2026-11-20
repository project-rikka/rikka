use crate::material::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EffectId {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EffectInstance {
    pub effect: EffectId,
    pub enabled: bool,
    pub values: Vec<(String, Value)>,
}

impl EffectInstance {
    pub fn new(effect: EffectId) -> Self {
        Self {
            effect,
            enabled: true,
            values: Vec::new(),
        }
    }

    pub fn set(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        let name = name.into();
        let value = value.into();
        match self.values.iter_mut().find(|(known, _)| *known == name) {
            Some(entry) => entry.1 = value,
            None => self.values.push((name, value)),
        }
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}
