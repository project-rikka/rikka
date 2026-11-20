use rikka_shader_programs::EffectLayout;
use rikka_slang::{ParameterSchema, Stage, TypeLayout};

use rikka_spirv::{ENTRY_POINT, Model};

use crate::effect::program::{parameter_schema, push_members};
use crate::error::{Error, Result};
use crate::shader::{MaterialShader, Tracing};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EffectInput {
    Input(String),
    Depth,
    Normals,
    Velocity,
    Layers,
    History,
    Emission,
    Target(String),
    Kept(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Wiring {
    #[default]
    Required,
    Optional,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EffectOutput {
    Image,
    Target(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectUniform {
    Size,
    Texel,
    Time,
    Frame,
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum EffectFormat {
    #[default]
    Rgba16Float,
    R32Float,
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum EffectSize {
    #[default]
    Full,
    Divided(u32),
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum TargetLifetime {
    #[default]
    Frame,
    Kept,
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct EffectTarget {
    pub format: EffectFormat,
    pub size: EffectSize,
    #[serde(default)]
    pub lifetime: TargetLifetime,
}

impl EffectTarget {
    pub fn new(format: EffectFormat, size: EffectSize) -> Self {
        Self {
            format,
            size,
            lifetime: TargetLifetime::Frame,
        }
    }

    pub fn kept(mut self) -> Self {
        self.lifetime = TargetLifetime::Kept;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectStage {
    Compute,
    Fragment,
}

#[derive(Clone, Debug)]
pub struct EffectCode {
    pub(crate) stage: EffectStage,
    pub(crate) shader: MaterialShader,
    pub(crate) workgroup: Option<[u32; 3]>,
    pub(crate) push: TypeLayout,
    pub(crate) parameters: Option<TypeLayout>,
}

impl EffectCode {
    pub(crate) fn compiled(spirv: Vec<u8>, layout: EffectLayout) -> Result<Self> {
        let stage = stage_of(layout.stage);
        let (shader, reflection) = MaterialShader::reflected(spirv, Tracing::Raster)?;
        let expected = match stage {
            EffectStage::Compute => Model::Compute,
            EffectStage::Fragment => Model::Fragment,
        };
        if reflection.models != [expected] {
            return Err(Error::InvalidShader(format!(
                "`{ENTRY_POINT}` is {:?}, the effect program declares a {expected:?} shader",
                reflection.models
            )));
        }
        Ok(Self {
            stage,
            shader,
            workgroup: layout.workgroup.or(reflection.local_size),
            push: layout.push,
            parameters: layout.parameters,
        })
    }

    pub(crate) fn builtin(program: &str, entry: &str) -> Self {
        let artifact = shaders_3d::EFFECTS
            .iter()
            .find(|artifact| artifact.program == program && artifact.entry == entry)
            .unwrap_or_else(|| panic!("built-in effect program {program} has no entry {entry}"));
        let layout = artifact.layout();
        Self {
            stage: stage_of(layout.stage),
            shader: MaterialShader::builtin(artifact.spirv),
            workgroup: layout.workgroup,
            push: layout.push,
            parameters: layout.parameters,
        }
    }

    pub fn stage(&self) -> EffectStage {
        self.stage
    }

    pub fn members(&self) -> impl Iterator<Item = &str> {
        push_members(&self.push)
            .unwrap_or_default()
            .iter()
            .map(|field| field.name.as_str())
    }
}

#[derive(Clone, Debug)]
#[must_use]
pub struct EffectStep {
    pub(crate) code: EffectCode,
    pub(crate) label: Option<String>,
    pub(crate) reads: Vec<(String, EffectInput)>,
    pub(crate) writes: Vec<(String, EffectOutput)>,
    pub(crate) output: Option<EffectOutput>,
    pub(crate) uniforms: Vec<(String, EffectUniform)>,
}

impl EffectStep {
    pub fn new(code: EffectCode) -> Self {
        Self {
            code,
            label: None,
            reads: Vec::new(),
            writes: Vec::new(),
            output: None,
            uniforms: Vec::new(),
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn read(mut self, member: impl Into<String>, input: EffectInput) -> Self {
        self.reads.push((member.into(), input));
        self
    }

    pub fn write(mut self, member: impl Into<String>, output: EffectOutput) -> Self {
        self.writes.push((member.into(), output));
        self
    }

    pub fn output(mut self, output: EffectOutput) -> Self {
        self.output = Some(output);
        self
    }

    pub fn uniform(mut self, member: impl Into<String>, uniform: EffectUniform) -> Self {
        self.uniforms.push((member.into(), uniform));
        self
    }
}

#[derive(Clone, Debug)]
#[must_use]
pub struct Effect {
    pub(crate) name: String,
    pub(crate) inputs: Vec<(String, Wiring)>,
    pub(crate) targets: Vec<(String, EffectTarget)>,
    pub(crate) steps: Vec<EffectStep>,
}

impl Effect {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            inputs: Vec::new(),
            targets: Vec::new(),
            steps: Vec::new(),
        }
    }

    pub fn input(mut self, socket: impl Into<String>) -> Self {
        self.inputs.push((socket.into(), Wiring::Required));
        self
    }

    pub fn optional(mut self, socket: impl Into<String>) -> Self {
        self.inputs.push((socket.into(), Wiring::Optional));
        self
    }

    pub fn target(mut self, name: impl Into<String>, target: EffectTarget) -> Self {
        self.targets.push((name.into(), target));
        self
    }

    pub fn step(mut self, step: EffectStep) -> Self {
        self.steps.push(step);
        self
    }

    pub fn inputs(&self) -> impl Iterator<Item = &str> {
        self.inputs.iter().map(|(name, _)| name.as_str())
    }

    pub fn sockets(&self) -> impl Iterator<Item = (&str, Wiring)> {
        self.inputs
            .iter()
            .map(|(name, wiring)| (name.as_str(), *wiring))
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn schema(&self) -> Result<ParameterSchema> {
        let parameters = self
            .steps
            .iter()
            .find_map(|step| step.code.parameters.as_ref());
        parameter_schema(&self.name, parameters)
    }
}

fn stage_of(stage: Stage) -> EffectStage {
    match stage {
        Stage::Fragment => EffectStage::Fragment,
        Stage::Compute => EffectStage::Compute,
        Stage::Vertex => unreachable!("effect programs are never vertex shaders"),
    }
}
