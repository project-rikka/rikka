use browser_translate::{Job, Result, Translation};

use crate::engine::Engine;

#[derive(Clone, Debug)]
pub struct Translator {
    inner: browser_translate::Translator,
}

impl Translator {
    pub fn new() -> Result<Translator> {
        Ok(Translator {
            inner: browser_translate::Translator::new()?,
        })
    }

    pub async fn translate(&self, engine: Engine, job: Job) -> Result<Translation> {
        self.inner.translate(engine.adapter(), job).await
    }
}
