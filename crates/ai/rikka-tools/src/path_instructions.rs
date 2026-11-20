use std::path::PathBuf;

pub trait PathInstructions: std::fmt::Debug + Send + Sync {
    fn instructions_for(&self, paths: &[PathBuf]) -> Option<String>;
}

#[derive(Debug)]
pub(crate) struct Nothing;

impl PathInstructions for Nothing {
    fn instructions_for(&self, _paths: &[PathBuf]) -> Option<String> {
        None
    }
}
