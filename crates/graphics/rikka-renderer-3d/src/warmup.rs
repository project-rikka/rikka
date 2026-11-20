use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::pipeline::{MeshPipelines, Recipe};
use rikka_spirv::Shader;

#[derive(Default)]
pub(crate) struct Warmup {
    pending: Option<Arc<AtomicUsize>>,
}

impl Warmup {
    pub(crate) fn is_started(&self) -> bool {
        self.pending.is_some()
    }

    pub(crate) fn start(
        &mut self,
        pipelines: &mut MeshPipelines,
        kernels: &[Shader],
        recipes: &[Recipe],
    ) {
        if self.is_started() {
            return;
        }
        let pending = Arc::new(AtomicUsize::new(kernels.len() + recipes.len()));
        let done = |pending: &Arc<AtomicUsize>| {
            let pending = Arc::clone(pending);
            move || {
                pending.fetch_sub(1, Ordering::AcqRel);
            }
        };
        for &shader in kernels {
            pipelines.when_kernel_ready(shader, done(&pending));
        }
        for recipe in recipes {
            pipelines.when_ready(recipe, done(&pending));
        }
        self.pending = Some(pending);
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|pending| pending.load(Ordering::Acquire) == 0)
    }
}
