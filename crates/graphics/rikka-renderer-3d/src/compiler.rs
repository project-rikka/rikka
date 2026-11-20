use std::hash::{Hash, Hasher};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;

use rikka_shader_programs::{CompileError, Variant};
use rikka_slang::{Compiler, Optimization};
use rikka_sync::Mutex;
use rustc_hash::{FxHashSet, FxHasher};

use crate::effect::EffectCode;
use crate::error::{Error, Result};
use crate::material::MaterialTemplate;
use crate::material::template::{Program, Variants, shaders};

const OPTIMIZER_THREAD: &str = "rikka-shader-optimize";

#[derive(Debug)]
pub struct ShaderCompiler {
    compilers: Arc<Compilers>,
}

#[derive(Debug)]
pub(crate) struct Compilers {
    optimized: Arc<Compiler>,
    pub draft: Compiler,
    probe: Compiler,
    probed: Mutex<FxHashSet<u64>>,
    optimizer: OnceLock<std::result::Result<Optimizer, String>>,
}

#[derive(Debug)]
struct Optimizer {
    jobs: Option<Sender<Job>>,
    thread: Option<JoinHandle<()>>,
}

struct Job {
    template: MaterialTemplate,
    base: Arc<Program>,
    variants: Variants,
}

impl ShaderCompiler {
    pub fn new() -> Result<Self> {
        let compiler = |optimization| {
            Compiler::builder()
                .modules(shaders_3d::SLANG_LIBRARY)
                .optimization(optimization)
                .build()
        };
        Ok(Self {
            compilers: Arc::new(Compilers {
                optimized: Arc::new(compiler(Optimization::High)?),
                draft: compiler(Optimization::None)?,
                probe: compiler(Optimization::None)?,
                probed: Mutex::new(FxHashSet::default()),
                optimizer: OnceLock::new(),
            }),
        })
    }

    pub fn template(&self, name: &str, source: &str) -> Result<MaterialTemplate> {
        MaterialTemplate::compile(self, name, source)
    }

    pub(crate) fn compilers(&self) -> &Arc<Compilers> {
        &self.compilers
    }

    pub fn effect_code(&self, name: &str, source: &str, entry: &str) -> Result<EffectCode> {
        let program =
            rikka_shader_programs::effect_program(&self.compilers.optimized, name, source, entry)
                .map_err(refused)?;
        EffectCode::compiled(rikka_spirv::bytes(&program.spirv), program.layout)
    }
}

impl Compilers {
    pub(crate) fn probe_unless_probed(&self, source: &str) -> Option<&Compiler> {
        (!self.probed.lock().contains(&digest(source))).then_some(&self.probe)
    }

    pub(crate) fn probed(&self, source: &str) {
        self.probed.lock().insert(digest(source));
    }

    pub(crate) fn optimize(
        &self,
        template: MaterialTemplate,
        base: Arc<Program>,
        variants: Variants,
    ) {
        let job = Job {
            template,
            base,
            variants,
        };
        let queue = self
            .optimizer
            .get_or_init(|| Optimizer::start(Arc::clone(&self.optimized)));
        let refused = match queue {
            Ok(optimizer) => optimizer
                .send(job)
                .map(|unsent| (unsent, "the shader optimizer stopped".to_owned())),
            Err(error) => Some((job, error.clone())),
        };
        if let Some((job, error)) = refused {
            job.template.fail(&job.base, Error::Invalid(error));
        }
    }
}

impl Optimizer {
    fn start(compiler: Arc<Compiler>) -> std::result::Result<Self, String> {
        let (jobs, receiver) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name(OPTIMIZER_THREAD.to_owned())
            .spawn(move || optimize(&compiler, receiver))
            .map(|thread| Self {
                jobs: Some(jobs),
                thread: Some(thread),
            })
            .map_err(|error| format!("failed to start the shader optimizer: {error}"))
    }

    fn send(&self, job: Job) -> Option<Job> {
        let jobs = self
            .jobs
            .as_ref()
            .expect("the optimizer takes jobs until dropped");
        jobs.send(job).err().map(|unsent| unsent.0)
    }
}

impl Drop for Optimizer {
    fn drop(&mut self) {
        drop(self.jobs.take());
        let Some(thread) = self.thread.take() else {
            return;
        };
        if thread.thread().id() == std::thread::current().id() {
            return;
        }
        if thread.join().is_err() {
            tracing::error!("the shader optimizer panicked");
        }
    }
}

fn optimize(compiler: &Compiler, jobs: Receiver<Job>) {
    for job in jobs {
        job.run(compiler);
    }
}

impl Job {
    fn run(self, compiler: &Compiler) {
        let origin = self
            .base
            .origin()
            .expect("only compiled templates are optimized");
        if self.template.generation() != self.base.generation {
            self.template.abandon(&self.base);
            return;
        }
        let variants: Vec<Variant> = self.variants.iter().collect();
        let built = rikka_shader_programs::compile_variants(
            compiler,
            &origin.name,
            &origin.source,
            &variants,
        )
        .map_err(refused)
        .and_then(|words| shaders(&variants, words));
        match built {
            Ok(replacements) => {
                let program = self.base.optimized(self.variants, replacements);
                self.template.optimized(&self.base, program);
            }
            Err(error) => self.template.fail(&self.base, error),
        }
    }
}

pub(crate) fn refused(error: CompileError) -> Error {
    match error {
        CompileError::Slang(error) => Error::Slang(error),
        CompileError::Schema(error) => Error::Schema(error),
        refused @ (CompileError::Copied(_)
        | CompileError::Unbounded
        | CompileError::VertexEffect(_)) => Error::Invalid(refused.to_string()),
    }
}

fn digest(source: &str) -> u64 {
    let mut hasher = FxHasher::default();
    source.hash(&mut hasher);
    hasher.finish()
}
