mod figure;
mod shell;
mod skeleton;

use std::sync::{Arc, LazyLock};

use rikka_model_core::Model;

static MODEL: LazyLock<Arc<Model>> = LazyLock::new(|| Arc::new(figure::assemble()));

pub fn model() -> Arc<Model> {
    Arc::clone(&MODEL)
}
