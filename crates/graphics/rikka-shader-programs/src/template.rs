use rikka_slang::{Compiler, ParameterSchema, Program, Role, SchemaError, Source, Type};

use crate::entries::{BLOCK, PROBE};
use crate::error::CompileError;
use crate::probe::copied_methods;
use crate::variant::Variant;

const SURFACE: &str = "Surface";

const MODEL: &str = "Model";

const VERTEX_INTERFACE: &str = "IVertex";

const SCATTERING_INTERFACE: &str = "IScattering";

#[derive(Debug)]
pub struct CompiledTemplate {
    pub schema: ParameterSchema,
    pub variants: Vec<Vec<u32>>,
    pub deforms: bool,
    pub scatters: bool,
}

pub fn compile_template(
    compiler: &Compiler,
    probe: Option<&Compiler>,
    name: &str,
    source: &str,
) -> Result<CompiledTemplate, CompileError> {
    build(compiler, probe, name, source, Source::new(name, source))
}

pub fn draft_template(
    compiler: &Compiler,
    probe: Option<&Compiler>,
    name: &str,
    source: &str,
) -> Result<CompiledTemplate, CompileError> {
    build(
        compiler,
        probe,
        name,
        source,
        Source::new(name, source).flatten(),
    )
}

pub fn compile_variants(
    compiler: &Compiler,
    name: &str,
    source: &str,
    variants: &[Variant],
) -> Result<Vec<Vec<u32>>, CompileError> {
    let request = variants
        .iter()
        .fold(Source::new(name, source), |request, variant| {
            request.entry(variant.entry())
        });
    let program = compiler.compile(&request)?;
    spirv_of(&program, variants)
}

fn build(
    compiler: &Compiler,
    probe: Option<&Compiler>,
    name: &str,
    source: &str,
    request: Source<'_>,
) -> Result<CompiledTemplate, CompileError> {
    let request = Variant::by_cost().fold(
        request
            .layout_of(BLOCK)
            .conformance(SURFACE, VERTEX_INTERFACE)
            .conformance(MODEL, SCATTERING_INTERFACE),
        |request, variant| request.entry(variant.entry()),
    );
    let (program, probed) = std::thread::scope(|scope| {
        let probing = probe.map(|probe| {
            scope.spawn(move || probe.compile(&Source::new(name, source).entry(PROBE)))
        });
        let program = compiler.compile(&request);
        let probed = probing.map(|probing| {
            probing
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
        });
        (program, probed)
    });
    let program = program?;
    let block = program
        .type_layout(BLOCK)
        .ok_or_else(|| rikka_slang::Error::UnknownType(BLOCK.to_owned()))?;
    let mut parts = block.ty.fields().iter();
    let (Some(surface), Some(model), Type::Struct { .. }) = (parts.next(), parts.next(), &block.ty)
    else {
        return Err(SchemaError::NotStruct(BLOCK.to_owned()).into());
    };
    let schema = ParameterSchema::from_struct(&surface.layout, surface.offset)?
        .merge(ParameterSchema::from_struct(&model.layout, model.offset)?)?;
    let owners = [&surface.layout.ty, &model.layout.ty].map(|ty| match ty {
        Type::Struct { name, .. } => name.as_str(),
        _ => "",
    });
    let deforms = program.conforms(SURFACE, VERTEX_INTERFACE) == Some(true);
    if deforms && schema.with_role(Role::Displacement).is_none() {
        return Err(CompileError::Unbounded);
    }
    let variants = spirv_of(&program, &Variant::ALL)?;
    let copied = match probed {
        Some(probed) => copied_methods(&probed?, owners),
        None => Vec::new(),
    };
    if !copied.is_empty() {
        return Err(CompileError::Copied(copied));
    }
    Ok(CompiledTemplate {
        schema,
        variants,
        deforms,
        scatters: program.conforms(MODEL, SCATTERING_INTERFACE) == Some(true),
    })
}

fn spirv_of(program: &Program, variants: &[Variant]) -> Result<Vec<Vec<u32>>, CompileError> {
    variants
        .iter()
        .map(|variant| {
            program
                .entry_point(variant.entry())
                .map(|entry| entry.spirv.clone())
                .ok_or_else(|| {
                    rikka_slang::Error::MissingEntryPoint(variant.entry().to_owned()).into()
                })
        })
        .collect()
}
