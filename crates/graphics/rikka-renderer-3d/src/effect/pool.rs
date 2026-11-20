use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::effect::desc::Effect;
use crate::effect::program::{self, Program, Shader};
use crate::effect::readiness::EffectReadiness;
use crate::effect::value::{EffectId, EffectInstance};
use crate::error::{Error, Resource, Result};
use crate::pipeline::MeshPipelines;
use crate::recipes;
use crate::state::Builtin;
use rikka_gpu::core::pipeline::Blend;
use rikka_gpu::texture::TexturePool;
use rikka_slang::ParameterSchema;

use crate::error::ParameterError;
use crate::material::value::{write_defaults, write_value};

pub(crate) const BLOCK_ALIGNMENT: usize = 16;

pub(crate) struct ResolvedEffect {
    pub program: Arc<Program>,
    pub parameters: u64,
}

struct Entry {
    generation: u32,
    program: Option<Arc<Program>>,
    pending: Option<Arc<Program>>,
    readiness: EffectReadiness,
}

#[derive(Default)]
pub(crate) struct EffectPool {
    entries: Vec<Entry>,
}

impl EffectPool {
    pub(crate) fn create(
        &mut self,
        effect: &Effect,
        pipelines: &mut MeshPipelines,
        builtin: &Builtin,
    ) -> Result<EffectId> {
        let program = Arc::new(program::compile(effect)?);
        let readiness = EffectReadiness::default();
        prepare(&program, &readiness, pipelines, builtin);
        let index = match self
            .entries
            .iter()
            .position(|entry| entry.program.is_none())
        {
            Some(index) => index,
            None => {
                self.entries.push(Entry {
                    generation: 0,
                    program: None,
                    pending: None,
                    readiness: EffectReadiness::default(),
                });
                self.entries.len() - 1
            }
        };
        let entry = &mut self.entries[index];
        entry.generation = entry.generation.wrapping_add(1);
        entry.program = Some(program);
        entry.pending = None;
        entry.readiness = readiness;
        Ok(EffectId {
            index: index as u32,
            generation: entry.generation,
        })
    }

    pub(crate) fn replace(
        &mut self,
        id: EffectId,
        effect: &Effect,
        pipelines: &mut MeshPipelines,
        builtin: &Builtin,
    ) -> Result<()> {
        self.program(id)?;
        let program = Arc::new(program::compile(effect)?);
        let entry = &mut self.entries[id.index as usize];
        prepare(&program, &entry.readiness, pipelines, builtin);
        entry.pending = Some(program);
        Ok(())
    }

    pub(crate) fn adopt_ready(&mut self) {
        for entry in &mut self.entries {
            if entry
                .pending
                .as_ref()
                .is_some_and(|pending| entry.readiness.reached(pending.generation))
            {
                entry.program = entry.pending.take();
            }
        }
    }

    pub(crate) fn destroy(&mut self, id: EffectId) -> bool {
        match self.entries.get_mut(id.index as usize) {
            Some(entry) if entry.generation == id.generation && entry.program.is_some() => {
                entry.program = None;
                entry.pending = None;
                true
            }
            _ => false,
        }
    }

    fn entry(&self, id: EffectId) -> Result<&Entry> {
        self.entries
            .get(id.index as usize)
            .filter(|entry| entry.generation == id.generation && entry.program.is_some())
            .ok_or(Error::Unknown(Resource::Effect(id)))
    }

    pub(crate) fn program(&self, id: EffectId) -> Result<&Arc<Program>> {
        Ok(self
            .entry(id)?
            .program
            .as_ref()
            .expect("a live entry has a program"))
    }

    pub(crate) fn schema(&self, id: EffectId) -> Result<ParameterSchema> {
        Ok(self.program(id)?.schema.clone())
    }

    pub(crate) fn readiness(&self, id: EffectId) -> Result<EffectReadiness> {
        Ok(self.entry(id)?.readiness.clone())
    }

    pub(crate) fn resolve(
        &self,
        instance: &EffectInstance,
        textures: &TexturePool,
        blocks: &mut Vec<u8>,
        slots: &mut Vec<u32>,
    ) -> Result<ResolvedEffect> {
        let program = self.program(instance.effect)?;
        let schema = &program.schema;
        let start = blocks.len().next_multiple_of(BLOCK_ALIGNMENT);
        blocks.resize(start + schema.size(), 0);
        let block = &mut blocks[start..];
        write_defaults(schema, block);
        for (name, value) in &instance.values {
            let Some(parameter) = schema.parameter(name) else {
                return Err(ParameterError::Unknown {
                    owner: program.name.to_string(),
                    name: name.clone(),
                }
                .into());
            };
            value.check(parameter)?;
            let slot = |texture| textures.slot(texture);
            slots.extend(write_value(parameter, value, block, &slot)?);
        }
        Ok(ResolvedEffect {
            program: program.clone(),
            parameters: start as u64,
        })
    }
}

fn prepare(
    program: &Program,
    readiness: &EffectReadiness,
    pipelines: &mut MeshPipelines,
    builtin: &Builtin,
) {
    let generation = program.generation;
    readiness.want(generation);
    let remaining = Arc::new(AtomicUsize::new(program.steps.len()));
    for step in &program.steps {
        let remaining = Arc::clone(&remaining);
        let readiness = readiness.clone();
        let built = move || {
            if remaining.fetch_sub(1, Ordering::AcqRel) == 1 {
                readiness.mark_ready(generation);
            }
        };
        match &step.shader {
            Shader::Kernel(shader) => pipelines.when_material_kernel_ready(shader, built),
            Shader::Fragment(shader) => pipelines.when_ready(
                &recipes::filter(builtin, shader, Blend::Opaque, step.format),
                built,
            ),
        }
    }
}
