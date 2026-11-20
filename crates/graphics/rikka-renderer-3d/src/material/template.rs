use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use rikka_shader_programs::{BuiltinTemplate, CompiledTemplate, TemplateArtifact, Variant};
use rikka_slang::ParameterSchema;
use rikka_sync::{Mutex, RwLock};

use crate::compiler::{Compilers, ShaderCompiler, refused};
use crate::error::{Error, Result};
use crate::material::progress::{Built, Progress, TemplateReadiness};
use crate::readiness::next_generation;
use crate::shader::{MaterialShader, Tracing};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Variants(u32);

impl Variants {
    const ALL: Self = Self((1 << Variant::COUNT) - 1);

    fn of(variant: Variant) -> Self {
        Self(1 << variant.index())
    }

    pub(crate) fn contains(self, variant: Variant) -> bool {
        self.0 & Self::of(variant).0 != 0
    }

    fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub(crate) fn iter(self) -> impl Iterator<Item = Variant> {
        Variant::ALL
            .into_iter()
            .filter(move |variant| self.contains(*variant))
    }
}

#[derive(Debug)]
pub(crate) struct Origin {
    pub compilers: Arc<Compilers>,
    pub name: String,
    pub source: String,
}

#[derive(Debug)]
pub(crate) struct Program {
    pub generation: u64,
    pub revision: u64,
    pub schema: Arc<ParameterSchema>,
    pub deforms: bool,
    pub scatters: bool,
    shaders: Vec<MaterialShader>,
    optimized: Variants,
    demanded: AtomicU32,
    requested: AtomicU32,
    origin: Option<Arc<Origin>>,
}

impl Program {
    pub(crate) fn shader(&self, variant: Variant) -> &MaterialShader {
        let bit = Variants::of(variant).0;
        if self.demanded.load(Ordering::Relaxed) & bit == 0 {
            self.demanded.fetch_or(bit, Ordering::Relaxed);
        }
        &self.shaders[variant.index()]
    }

    pub(crate) fn shaders(&self) -> &[MaterialShader] {
        &self.shaders
    }

    pub(crate) fn origin(&self) -> Option<&Arc<Origin>> {
        self.origin.as_ref()
    }

    fn demanded(&self) -> Variants {
        Variants(self.demanded.load(Ordering::Relaxed))
    }

    fn unoptimized(&self) -> Variants {
        self.demanded().without(self.optimized)
    }

    fn unrequested(&self) -> Variants {
        self.unoptimized()
            .without(Variants(self.requested.load(Ordering::Relaxed)))
    }

    pub(crate) fn wants_optimizing(&self) -> bool {
        self.origin.is_some() && !self.unrequested().is_empty()
    }

    pub(crate) fn optimized(
        &self,
        variants: Variants,
        replacements: Vec<MaterialShader>,
    ) -> Arc<Self> {
        let mut shaders = self.shaders.clone();
        for (variant, shader) in variants.iter().zip(replacements) {
            shaders[variant.index()] = shader;
        }
        Arc::new(Self {
            generation: next_generation(),
            revision: self.revision,
            schema: Arc::clone(&self.schema),
            deforms: self.deforms,
            scatters: self.scatters,
            shaders,
            optimized: self.optimized.with(variants),
            demanded: AtomicU32::new(self.demanded().0),
            requested: AtomicU32::new(0),
            origin: self.origin.clone(),
        })
    }
}

#[derive(Debug)]
struct Shared {
    name: String,
    builtin: bool,
    revisions: AtomicU64,
    program: RwLock<Arc<Program>>,
    progress: Mutex<Progress>,
}

#[derive(Clone, Debug)]
pub struct MaterialTemplate {
    shared: Arc<Shared>,
}

impl PartialEq for MaterialTemplate {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }
}

impl Eq for MaterialTemplate {}

impl MaterialTemplate {
    pub fn builtin(builtin: BuiltinTemplate) -> Self {
        static TEMPLATES: OnceLock<Vec<MaterialTemplate>> = OnceLock::new();
        let templates = TEMPLATES.get_or_init(|| {
            BuiltinTemplate::ALL
                .iter()
                .map(|builtin| {
                    Self::prebuilt(builtin.id(), &shaders_3d::TEMPLATES[builtin.index()])
                })
                .collect()
        });
        templates[builtin.index()].clone()
    }

    pub fn prebuilt(name: &str, artifact: &TemplateArtifact) -> Self {
        let manifest = artifact.manifest();
        let shaders = artifact
            .variants
            .iter()
            .map(|shader| MaterialShader::builtin(*shader))
            .collect();
        Self::from_program(
            name,
            true,
            Arc::new(Program {
                generation: next_generation(),
                revision: 1,
                schema: Arc::new(manifest.schema),
                deforms: manifest.deforms,
                scatters: manifest.scatters,
                shaders,
                optimized: Variants::ALL,
                demanded: AtomicU32::new(0),
                requested: AtomicU32::new(0),
                origin: None,
            }),
        )
    }

    pub(crate) fn compile(compiler: &ShaderCompiler, name: &str, source: &str) -> Result<Self> {
        let program = draft(compiler.compilers(), name, source, 1)?;
        Ok(Self::from_program(name, false, program))
    }

    fn from_program(name: &str, builtin: bool, program: Arc<Program>) -> Self {
        Self {
            shared: Arc::new(Shared {
                name: name.to_owned(),
                builtin,
                revisions: AtomicU64::new(program.revision),
                program: RwLock::new(program),
                progress: Mutex::new(Progress::default()),
            }),
        }
    }

    pub fn reload(&self, compiler: &ShaderCompiler, source: &str) -> Result<()> {
        if self.shared.builtin {
            return Err(Error::Invalid(format!(
                "built-in template {} cannot be reloaded",
                self.shared.name
            )));
        }
        let revision = self.shared.revisions.fetch_add(1, Ordering::AcqRel) + 1;
        let program = draft(compiler.compilers(), &self.shared.name, source, revision)?;
        let mut current = self.shared.program.write();
        if current.revision < program.revision {
            *current = program;
        }
        Ok(())
    }

    pub fn name(&self) -> &str {
        &self.shared.name
    }

    pub fn schema(&self) -> Arc<ParameterSchema> {
        Arc::clone(&self.program().schema)
    }

    pub(crate) fn program(&self) -> Arc<Program> {
        Arc::clone(&self.shared.program.read())
    }

    pub fn generation(&self) -> u64 {
        self.shared.program.read().generation
    }

    pub fn is_ready(&self) -> bool {
        let revision = self.program().revision;
        self.shared.progress.lock().drawable >= revision
    }

    pub fn on_ready(&self, notify: impl FnOnce() + Send + 'static) {
        let revision = self.program().revision;
        let mut progress = self.shared.progress.lock();
        if progress.drawable >= revision {
            drop(progress);
            notify();
            return;
        }
        progress.wait_drawable(revision, Box::new(notify));
    }

    pub fn readiness(&self) -> TemplateReadiness {
        let program = self.program();
        let progress = self.shared.progress.lock();
        readiness(&program, &progress)
    }

    pub fn on_built(&self, notify: impl FnOnce(Result<(), Arc<Error>>) + Send + 'static) {
        let program = self.program();
        let mut progress = self.shared.progress.lock();
        let settled = match readiness(&program, &progress) {
            TemplateReadiness::Built => Some(Ok(())),
            TemplateReadiness::Failed(error) => Some(Err(error)),
            TemplateReadiness::Pending | TemplateReadiness::Drawable => None,
        };
        match settled {
            Some(result) => {
                drop(progress);
                notify(result);
            }
            None => progress.wait_built(program.revision, Box::new(notify)),
        }
    }

    pub fn watch(&self, notify: impl FnOnce() + Send + 'static) -> TemplateReadiness {
        let mut progress = self.shared.progress.lock();
        let readiness = readiness(&self.program(), &progress);
        if matches!(
            readiness,
            TemplateReadiness::Pending | TemplateReadiness::Drawable
        ) {
            progress.wait_step(Box::new(notify));
        }
        readiness
    }

    pub(crate) fn mark_ready(&self, program: &Arc<Program>) {
        let current = self.program();
        let mut progress = self.shared.progress.lock();
        progress.drawable = progress.drawable.max(program.revision);
        if current.generation == program.generation {
            progress.ready = program.generation;
        }
        let built = matches!(readiness(&current, &progress), TemplateReadiness::Built)
            .then_some((current.revision, Ok(())));
        let due = progress.settle(built);
        drop(progress);
        due.notify();
    }

    pub(crate) fn optimize(&self, program: &Arc<Program>) {
        let Some(origin) = program.origin() else {
            return;
        };
        let current = self.program();
        if current.generation != program.generation {
            if current.revision == program.revision {
                current
                    .demanded
                    .fetch_or(program.demanded().0, Ordering::Relaxed);
            }
            return;
        }
        let mut progress = self.shared.progress.lock();
        if progress.ready != program.generation
            || progress.optimizing == program.generation
            || progress.failure(program.revision).is_some()
        {
            return;
        }
        let variants = program.unrequested();
        if variants.is_empty() {
            return;
        }
        program.requested.fetch_or(variants.0, Ordering::Relaxed);
        progress.optimizing = program.generation;
        drop(progress);
        origin
            .compilers
            .optimize(self.clone(), Arc::clone(program), variants);
    }

    pub(crate) fn optimized(&self, base: &Program, program: Arc<Program>) {
        {
            let mut current = self.shared.program.write();
            if current.generation == base.generation {
                *current = program;
            }
        }
        let mut progress = self.shared.progress.lock();
        if progress.optimizing == base.generation {
            progress.optimizing = 0;
        }
        let due = progress.step();
        drop(progress);
        due.notify();
    }

    pub(crate) fn abandon(&self, base: &Program) {
        self.finish_optimizing(base);
    }

    pub(crate) fn fail(&self, base: &Program, error: Error) {
        let error = Arc::new(error);
        let mut progress = self.shared.progress.lock();
        if progress.optimizing == base.generation {
            progress.optimizing = 0;
        }
        progress.failed = Some((base.revision, Arc::clone(&error)));
        let due = progress.settle(Some((base.revision, Err(error) as Built)));
        drop(progress);
        due.notify();
    }

    fn finish_optimizing(&self, base: &Program) {
        let mut progress = self.shared.progress.lock();
        if progress.optimizing == base.generation {
            progress.optimizing = 0;
        }
    }
}

fn readiness(program: &Program, progress: &Progress) -> TemplateReadiness {
    if let Some(error) = progress.failure(program.revision) {
        return TemplateReadiness::Failed(Arc::clone(error));
    }
    if progress.drawable < program.revision {
        return TemplateReadiness::Pending;
    }
    match progress.ready == program.generation && program.unoptimized().is_empty() {
        true => TemplateReadiness::Built,
        false => TemplateReadiness::Drawable,
    }
}

pub(crate) fn shaders(variants: &[Variant], words: Vec<Vec<u32>>) -> Result<Vec<MaterialShader>> {
    words
        .into_iter()
        .zip(variants)
        .map(|(words, variant)| {
            let tracing = match variant.traces_rays() {
                true => Tracing::RayQueries,
                false => Tracing::Raster,
            };
            MaterialShader::from_spirv(rikka_spirv::bytes(&words), tracing)
        })
        .collect()
}

fn draft(
    compilers: &Arc<Compilers>,
    name: &str,
    source: &str,
    revision: u64,
) -> Result<Arc<Program>> {
    let wrapped = rikka_shader_programs::source(source);
    let CompiledTemplate {
        schema,
        variants,
        deforms,
        scatters,
    } = rikka_shader_programs::draft_template(
        &compilers.draft,
        compilers.probe_unless_probed(&wrapped),
        name,
        &wrapped,
    )
    .map_err(refused)?;
    let shaders = shaders(&Variant::ALL, variants)?;
    compilers.probed(&wrapped);
    Ok(Arc::new(Program {
        generation: next_generation(),
        revision,
        schema: Arc::new(schema),
        deforms,
        scatters,
        shaders,
        optimized: Variants::default(),
        demanded: AtomicU32::new(0),
        requested: AtomicU32::new(0),
        origin: Some(Arc::new(Origin {
            compilers: Arc::clone(compilers),
            name: name.to_owned(),
            source: wrapped,
        })),
    }))
}
