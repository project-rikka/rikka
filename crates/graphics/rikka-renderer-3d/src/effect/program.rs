use ash::vk;
use rikka_slang::{Field as Member, Scalar, Type, TypeLayout};

use crate::effect::desc::{
    Effect, EffectFormat, EffectInput, EffectOutput, EffectSize, EffectStage, EffectStep,
    EffectUniform, TargetLifetime, Wiring,
};
use crate::effect::label::intern;
use crate::error::{Error, Result};
use crate::readiness::next_generation;
use crate::shader::MaterialShader;
use crate::state::PUSH_SIZE;
use crate::viewport::COLOR_FORMAT;
use rikka_slang::ParameterSchema;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Input(usize),
    Depth,
    Normals,
    Velocity,
    Layers,
    History,
    Emission,
    Target(usize),
    Kept(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sink {
    Image,
    Target(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Field {
    Read(Source),
    Write(Sink),
    Uniform(EffectUniform),
    Parameters,
}

#[derive(Clone, Debug)]
pub(crate) enum Shader {
    Kernel(MaterialShader),
    Fragment(MaterialShader),
}

#[derive(Clone, Debug)]
pub(crate) struct Step {
    pub label: &'static str,
    pub shader: Shader,
    pub fields: Vec<(u32, Field)>,
    pub sinks: Vec<Sink>,
    pub sources: Vec<Source>,
    pub local: [u32; 2],
    pub format: vk::Format,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Target {
    pub format: vk::Format,
    pub divisor: u32,
    pub lifetime: TargetLifetime,
}

#[derive(Debug)]
pub(crate) struct Program {
    pub generation: u64,
    pub name: &'static str,
    pub inputs: Vec<(&'static str, Wiring)>,
    pub targets: Vec<Target>,
    pub steps: Vec<Step>,
    pub schema: ParameterSchema,
}

impl Program {
    pub(crate) fn reads(&self, source: Source) -> bool {
        self.steps.iter().any(|step| step.sources.contains(&source))
    }

    pub(crate) fn primary(&self) -> Option<&'static str> {
        self.inputs.first().map(|&(name, _)| name)
    }
}

fn format_of(format: EffectFormat) -> vk::Format {
    match format {
        EffectFormat::Rgba16Float => vk::Format::R16G16B16A16_SFLOAT,
        EffectFormat::R32Float => vk::Format::R32_SFLOAT,
    }
}

fn uniform_fits(uniform: EffectUniform, ty: &Type) -> bool {
    match uniform {
        EffectUniform::Size => matches!(
            ty,
            Type::Vector {
                scalar: Scalar::U32,
                count: 2
            }
        ),
        EffectUniform::Texel => matches!(
            ty,
            Type::Vector {
                scalar: Scalar::F32,
                count: 2
            }
        ),
        EffectUniform::Time => matches!(ty, Type::Scalar(Scalar::F32)),
        EffectUniform::Frame => matches!(ty, Type::Scalar(Scalar::U64) | Type::Pointer { .. }),
    }
}

pub(crate) fn push_members(layout: &TypeLayout) -> Option<&[Member]> {
    match &layout.ty {
        Type::ConstantBuffer(inner) => push_members(inner),
        Type::Struct { fields, .. } => Some(fields),
        _ => None,
    }
}

fn struct_name(layout: &TypeLayout) -> Option<&str> {
    match &layout.ty {
        Type::Struct { name, .. } => Some(name),
        _ => None,
    }
}

struct Compiler<'a> {
    effect: &'a Effect,
    parameters: Option<&'a TypeLayout>,
    written: Vec<bool>,
}

impl<'a> Compiler<'a> {
    fn fail(&self, message: impl std::fmt::Display) -> Error {
        Error::Invalid(format!("effect `{}`: {message}", self.effect.name))
    }

    fn target(&self, name: &str) -> Result<usize> {
        self.effect
            .targets
            .iter()
            .position(|(known, _)| known == name)
            .ok_or_else(|| self.fail(format_args!("target `{name}` is not declared")))
    }

    fn source(&self, input: &EffectInput) -> Result<Source> {
        Ok(match input {
            EffectInput::Input(name) => Source::Input(
                self.effect
                    .inputs
                    .iter()
                    .position(|(known, _)| known == name)
                    .ok_or_else(|| self.fail(format_args!("input `{name}` is not declared")))?,
            ),
            EffectInput::Depth => Source::Depth,
            EffectInput::Normals => Source::Normals,
            EffectInput::Velocity => Source::Velocity,
            EffectInput::Layers => Source::Layers,
            EffectInput::History => Source::History,
            EffectInput::Emission => Source::Emission,
            EffectInput::Target(name) => {
                let index = self.target(name)?;
                if !self.written[index] {
                    return Err(self.fail(format_args!(
                        "target `{name}` is read before a step writes it"
                    )));
                }
                Source::Target(index)
            }
            EffectInput::Kept(name) => {
                let index = self.target(name)?;
                if self.effect.targets[index].1.lifetime != TargetLifetime::Kept {
                    return Err(self.fail(format_args!(
                        "target `{name}` is read from the last frame but is not kept"
                    )));
                }
                Source::Kept(index)
            }
        })
    }

    fn sink(&self, output: &EffectOutput) -> Result<Sink> {
        Ok(match output {
            EffectOutput::Image => Sink::Image,
            EffectOutput::Target(name) => Sink::Target(self.target(name)?),
        })
    }

    fn sink_format(&self, sink: Sink) -> vk::Format {
        match sink {
            Sink::Image => COLOR_FORMAT,
            Sink::Target(index) => format_of(self.effect.targets[index].1.format),
        }
    }

    fn adopt_parameters(&mut self, at: usize, layout: &'a TypeLayout) -> Result<()> {
        match self.parameters {
            Some(known) if known != layout => Err(self.fail(format_args!(
                "step {at} declares a different parameter block than an earlier step"
            ))),
            _ => {
                self.parameters = Some(layout);
                Ok(())
            }
        }
    }

    fn step(&mut self, at: usize, step: &'a EffectStep) -> Result<Step> {
        let code = &step.code;
        let Some(members) = push_members(&code.push) else {
            return Err(self.fail(format_args!("step {at}: push constants are not a struct")));
        };
        if code.push.size > PUSH_SIZE as usize {
            return Err(self.fail(format_args!(
                "step {at}: push constants use {} bytes, limit is {PUSH_SIZE}",
                code.push.size
            )));
        }
        if let Some(layout) = &code.parameters {
            self.adopt_parameters(at, layout)?;
        }
        let member = |name: &str| -> Result<&Member> {
            members
                .iter()
                .find(|member| member.name == name)
                .ok_or_else(|| {
                    self.fail(format_args!(
                        "step {at}: push constants have no member `{name}`"
                    ))
                })
        };
        let slot = |name: &str| -> Result<u32> {
            let found = member(name)?;
            match found.layout.ty {
                Type::Scalar(Scalar::U32) => Ok(found.offset as u32),
                _ => Err(self.fail(format_args!(
                    "step {at}: member `{name}` binds an image and must be a uint"
                ))),
            }
        };

        let mut fields: Vec<(u32, Field)> = Vec::new();
        let mut bound: Vec<&str> = Vec::new();
        let mut claim = |name: &'a str| -> Result<()> {
            if bound.contains(&name) {
                return Err(self.fail(format_args!("step {at}: member `{name}` is bound twice")));
            }
            bound.push(name);
            Ok(())
        };
        let mut sources = Vec::new();
        for (name, input) in &step.reads {
            claim(name)?;
            let source = self.source(input)?;
            fields.push((slot(name)?, Field::Read(source)));
            sources.push(source);
        }
        let mut sinks = Vec::new();
        match (code.stage, &step.output) {
            (EffectStage::Compute, Some(_)) => {
                return Err(self.fail(format_args!(
                    "step {at}: compute steps name their outputs with write()"
                )));
            }
            (EffectStage::Compute, None) if step.writes.is_empty() => {
                return Err(self.fail(format_args!("step {at}: a compute step writes nothing")));
            }
            (EffectStage::Fragment, None) => {
                return Err(self.fail(format_args!("step {at}: a fragment step needs an output()")));
            }
            (EffectStage::Fragment, Some(_)) if !step.writes.is_empty() => {
                return Err(self.fail(format_args!(
                    "step {at}: fragment steps write only their output attachment"
                )));
            }
            (EffectStage::Fragment, Some(output)) => sinks.push(self.sink(output)?),
            (EffectStage::Compute, None) => {}
        }
        for (name, output) in &step.writes {
            claim(name)?;
            let sink = self.sink(output)?;
            fields.push((slot(name)?, Field::Write(sink)));
            sinks.push(sink);
        }
        for sink in &sinks {
            if let Sink::Target(index) = *sink
                && sources.contains(&Source::Target(index))
            {
                return Err(self.fail(format_args!(
                    "step {at}: target `{}` is both read and written",
                    self.effect.targets[index].0
                )));
            }
        }
        for (name, uniform) in &step.uniforms {
            claim(name)?;
            let found = member(name)?;
            if !uniform_fits(*uniform, &found.layout.ty) {
                return Err(self.fail(format_args!(
                    "step {at}: member `{name}` cannot hold {uniform:?}"
                )));
            }
            fields.push((found.offset as u32, Field::Uniform(*uniform)));
        }
        let block = code.parameters.as_ref().and_then(struct_name);
        for found in members
            .iter()
            .filter(|member| !bound.contains(&member.name.as_str()))
        {
            match (&found.layout.ty, block) {
                (Type::Pointer { pointee }, Some(block)) if pointee == block => {
                    fields.push((found.offset as u32, Field::Parameters));
                }
                (Type::Pointer { pointee }, _) => {
                    return Err(self.fail(format_args!(
                        "step {at}: member `{}` points at {pointee}, which is not the effect's \
                         parameter block",
                        found.name
                    )));
                }
                _ => {
                    return Err(self.fail(format_args!(
                        "step {at}: push member `{}` is not bound; parameters belong in the \
                         parameter block",
                        found.name
                    )));
                }
            }
        }
        for sink in &sinks {
            if let Sink::Target(index) = *sink {
                self.written[index] = true;
            }
        }

        let local = match code.stage {
            EffectStage::Compute => match code.workgroup {
                Some([x, y, 1]) if x > 0 && y > 0 => [x, y],
                Some(size) => {
                    return Err(self.fail(format_args!(
                        "step {at}: workgroup size {size:?} must be two-dimensional"
                    )));
                }
                None => {
                    return Err(self.fail(format_args!(
                        "step {at}: the compute shader declares no fixed workgroup size"
                    )));
                }
            },
            EffectStage::Fragment => [1, 1],
        };
        let shader = match code.stage {
            EffectStage::Compute => Shader::Kernel(code.shader.clone()),
            EffectStage::Fragment => Shader::Fragment(code.shader.clone()),
        };
        let label = intern(&match &step.label {
            Some(label) => format!("rikka.effect.{}.{label}", self.effect.name),
            None => format!("rikka.effect.{}.{at}", self.effect.name),
        });
        Ok(Step {
            label,
            shader,
            fields,
            format: self.sink_format(sinks[0]),
            sinks,
            sources,
            local,
        })
    }
}

pub(crate) fn compile(effect: &Effect) -> Result<Program> {
    if effect.steps.is_empty() {
        return Err(Error::Invalid(format!(
            "effect `{}` has no steps",
            effect.name
        )));
    }
    let mut compiler = Compiler {
        effect,
        parameters: None,
        written: vec![false; effect.targets.len()],
    };
    for (at, (name, _)) in effect.inputs.iter().enumerate() {
        if effect.inputs[..at].iter().any(|(known, _)| known == name) {
            return Err(compiler.fail(format_args!("input `{name}` is declared twice")));
        }
    }
    if let Some((name, Wiring::Optional)) = effect.inputs.first() {
        return Err(compiler.fail(format_args!(
            "the first input `{name}` passes the image through and cannot be optional"
        )));
    }
    for (at, (name, target)) in effect.targets.iter().enumerate() {
        if effect.targets[..at].iter().any(|(known, _)| known == name) {
            return Err(compiler.fail(format_args!("target `{name}` is declared twice")));
        }
        if target.size == EffectSize::Divided(0) {
            return Err(compiler.fail(format_args!("target `{name}` is divided by zero")));
        }
    }
    let steps = effect
        .steps
        .iter()
        .enumerate()
        .map(|(at, step)| compiler.step(at, step))
        .collect::<Result<Vec<_>>>()?;
    let writes = steps
        .iter()
        .flat_map(|step| &step.sinks)
        .filter(|&&sink| sink == Sink::Image)
        .count();
    match writes {
        0 => return Err(compiler.fail("no step writes the image")),
        1 => {}
        _ => return Err(compiler.fail("the image is written more than once")),
    }
    let reads_history = steps.iter().any(|step| {
        step.sources
            .iter()
            .any(|source| matches!(source, Source::History | Source::Kept(_)))
    });
    if reads_history && effect.inputs.is_empty() {
        return Err(compiler.fail("an effect reading its history declares no input"));
    }
    for (index, (name, target)) in effect.targets.iter().enumerate() {
        if target.lifetime == TargetLifetime::Kept && !compiler.written[index] {
            return Err(compiler.fail(format_args!("kept target `{name}` is never written")));
        }
    }
    let schema = parameter_schema(&effect.name, compiler.parameters)?;
    Ok(Program {
        generation: next_generation(),
        name: intern(&effect.name),
        inputs: effect
            .inputs
            .iter()
            .map(|(name, wiring)| (intern(name), *wiring))
            .collect(),
        targets: effect
            .targets
            .iter()
            .map(|(_, target)| Target {
                format: format_of(target.format),
                divisor: match target.size {
                    EffectSize::Full => 1,
                    EffectSize::Divided(divisor) => divisor,
                },
                lifetime: target.lifetime,
            })
            .collect(),
        steps,
        schema,
    })
}

pub(crate) fn parameter_schema(
    effect: &str,
    parameters: Option<&TypeLayout>,
) -> Result<ParameterSchema> {
    match parameters {
        Some(layout) => ParameterSchema::from_struct(layout, 0)
            .map_err(|error| Error::Invalid(format!("effect `{effect}`: {error}"))),
        None => Ok(ParameterSchema::default()),
    }
}
