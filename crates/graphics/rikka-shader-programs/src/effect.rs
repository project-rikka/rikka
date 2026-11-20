use rikka_slang::{Binding, Compiler, EntryPoint, Program, Source, Stage, Type, TypeLayout};
use serde::{Deserialize, Serialize};

use crate::error::CompileError;

pub const PARAMETERS: &str = "Parameters";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectLayout {
    pub stage: Stage,
    pub workgroup: Option<[u32; 3]>,
    pub push: TypeLayout,
    pub parameters: Option<TypeLayout>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EffectProgram {
    pub entry: String,
    pub spirv: Vec<u32>,
    pub layout: EffectLayout,
}

pub fn effect_program(
    compiler: &Compiler,
    name: &str,
    source: &str,
    entry: &str,
) -> Result<EffectProgram, CompileError> {
    let program =
        compiler.compile(&Source::new(name, source).entry(entry).layout_of(PARAMETERS))?;
    let point = program
        .entry_point(entry)
        .ok_or_else(|| rikka_slang::Error::MissingEntryPoint(entry.to_owned()))?;
    assembled(&program, point)
}

pub fn effect_programs(
    compiler: &Compiler,
    name: &str,
    source: &str,
) -> Result<Vec<EffectProgram>, CompileError> {
    let program = compiler.compile(&Source::new(name, source).layout_of(PARAMETERS))?;
    program
        .entry_points()
        .iter()
        .map(|point| assembled(&program, point))
        .collect()
}

fn assembled(program: &Program, point: &EntryPoint) -> Result<EffectProgram, CompileError> {
    if point.stage == Stage::Vertex {
        return Err(CompileError::VertexEffect(point.name.clone()));
    }
    let push = program
        .parameters()
        .iter()
        .find(|parameter| parameter.binding == Binding::PushConstant)
        .map_or_else(
            || TypeLayout {
                ty: Type::Struct {
                    name: String::new(),
                    fields: Vec::new(),
                },
                size: 0,
            },
            |parameter| parameter.layout.clone(),
        );
    Ok(EffectProgram {
        entry: point.name.clone(),
        spirv: point.spirv.clone(),
        layout: EffectLayout {
            stage: point.stage,
            workgroup: point.workgroup_size,
            push,
            parameters: program.type_layout(PARAMETERS).cloned(),
        },
    })
}
