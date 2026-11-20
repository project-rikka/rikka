use std::sync::Arc;

use rikka_slang::ParameterSchema;

use crate::error::{ParameterError, Result};
use crate::material::state::MaterialState;
use crate::material::template::{MaterialTemplate, Program};
use crate::material::value::Value;

#[derive(Clone, Debug)]
#[must_use]
pub struct MaterialDesc {
    pub(crate) template: MaterialTemplate,
    pub(crate) program: Arc<Program>,
    pub(crate) values: Vec<Value>,
    pub(crate) state: MaterialState,
    pub(crate) alpha_cutoff: f32,
    pub(crate) hull: bool,
}

impl MaterialDesc {
    pub fn new(template: &MaterialTemplate) -> Self {
        let program = template.program();
        let values = defaults(&program.schema);
        Self {
            template: template.clone(),
            program,
            values,
            state: MaterialState::default(),
            alpha_cutoff: 0.5,
            hull: false,
        }
    }

    pub fn template(&self) -> &MaterialTemplate {
        &self.template
    }

    pub fn schema(&self) -> &ParameterSchema {
        &self.program.schema
    }

    pub fn to_template(&self, template: &MaterialTemplate) -> Self {
        let mut desc = Self::new(template)
            .state(self.state)
            .alpha_cutoff(self.alpha_cutoff);
        desc.hull = self.hull;
        for (index, spec) in desc.program.schema.parameters().iter().enumerate() {
            let by_role = || {
                let role = spec.role?;
                let previous = self.program.schema.with_role(role)?;
                self.value(&previous.name)
            };
            if let Some(previous) = self
                .value(&spec.name)
                .filter(|previous| previous.check(spec).is_ok())
                .or_else(|| by_role().filter(|previous| previous.check(spec).is_ok()))
            {
                desc.values[index] = previous.clone();
            }
        }
        desc
    }

    pub fn state(mut self, state: MaterialState) -> Self {
        self.state = state;
        self
    }

    pub fn alpha_cutoff(mut self, cutoff: f32) -> Self {
        self.alpha_cutoff = cutoff;
        self
    }

    pub fn hull(mut self) -> Self {
        self.hull = true;
        self
    }

    pub fn set(&mut self, name: &str, value: impl Into<Value>) -> Result<&mut Self> {
        let index = self.index_of(name)?;
        let value = value.into();
        value.check(&self.program.schema.parameters()[index])?;
        self.values[index] = value;
        Ok(self)
    }

    pub fn with(mut self, name: &str, value: impl Into<Value>) -> Result<Self> {
        self.set(name, value)?;
        Ok(self)
    }

    pub fn preset(mut self, preset: &str) -> Result<Self> {
        let schema = Arc::clone(&self.program.schema);
        if !schema.presets().contains(preset) {
            return Err(ParameterError::UnknownPreset {
                owner: self.template.name().to_owned(),
                name: preset.to_owned(),
            }
            .into());
        }
        for (index, spec) in schema.parameters().iter().enumerate() {
            if let Some(literal) = spec.preset(preset) {
                self.values[index] = Value::from_literal(spec, literal);
            }
        }
        Ok(self)
    }

    pub fn value(&self, name: &str) -> Option<&Value> {
        self.program
            .schema
            .index_of(name)
            .map(|index| &self.values[index])
    }

    fn index_of(&self, name: &str) -> Result<usize, ParameterError> {
        self.program
            .schema
            .index_of(name)
            .ok_or_else(|| ParameterError::Unknown {
                owner: self.template.name().to_owned(),
                name: name.to_owned(),
            })
    }

    pub(crate) fn migrate(&mut self, program: Arc<Program>) {
        let mut values = defaults(&program.schema);
        for (index, spec) in program.schema.parameters().iter().enumerate() {
            if let Some(previous) = self.value(&spec.name)
                && previous.check(spec).is_ok()
            {
                values[index] = previous.clone();
            }
        }
        self.values = values;
        self.program = program;
    }
}

pub(crate) fn defaults(schema: &ParameterSchema) -> Vec<Value> {
    schema
        .parameters()
        .iter()
        .map(|spec| Value::from_literal(spec, spec.default))
        .collect()
}
