use std::sync::Arc;

use rikka_renderer_3d::ShaderCompiler;
use rikka_shader_compile::Material;
use rikka_shader_graph::{Graph, Library};

use crate::diagnostic::{self, Diagnostic};

#[derive(Clone, Debug)]
pub struct Job {
    pub key: String,
    pub revision: u64,
    pub(crate) name: String,
    pub(crate) signature: Arc<str>,
    pub(crate) graph: Graph,
    pub(crate) library: Arc<Library>,
    pub(crate) material: Option<Arc<Material>>,
}

#[derive(Debug)]
pub struct Outcome {
    pub key: String,
    pub revision: u64,
    pub(crate) signature: Arc<str>,
    pub result: Result<Material, Vec<Diagnostic>>,
}

impl Job {
    pub fn run(self, compiler: &ShaderCompiler) -> Outcome {
        let lines = self.signature.lines().count();
        let module = self
            .material
            .as_ref()
            .map_or_else(|| self.name.clone(), |live| live.source().name.clone());
        let result = match self.material {
            Some(live) => {
                let mut material = Material::clone(&live);
                material
                    .reload(&self.graph, &self.library, compiler)
                    .map(|()| material)
            }
            None => Material::compile(&self.graph, &self.library, compiler, &self.name),
        }
        .map_err(|error| diagnostic::from_build(&error, &module, lines));
        Outcome {
            key: self.key,
            revision: self.revision,
            signature: self.signature,
            result,
        }
    }
}
