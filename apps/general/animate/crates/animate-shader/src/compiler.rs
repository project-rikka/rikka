use std::sync::{Arc, OnceLock};

use rikka_renderer_3d::ShaderCompiler;

type Made = Result<Arc<ShaderCompiler>, Arc<str>>;

#[derive(Clone, Default)]
pub struct SharedCompiler {
    made: Arc<OnceLock<Made>>,
}

impl SharedCompiler {
    pub fn get(&self) -> Made {
        self.made
            .get_or_init(|| {
                ShaderCompiler::new()
                    .map(Arc::new)
                    .map_err(|error| error.to_string().into())
            })
            .clone()
    }
}

impl std::fmt::Debug for SharedCompiler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedCompiler")
            .field("made", &self.made.get().map(Result::is_ok))
            .finish()
    }
}
