use std::path::{Path, PathBuf};

use crate::config::aliases::Aliases;
use crate::config::environment::Environment;
use crate::config::glob::Glob;
use crate::config::loader::Loader;
use crate::report::LoadProblem;

#[derive(Debug, Default)]
pub(crate) struct Fontconfig {
    pub(crate) dirs: Vec<PathBuf>,
    pub(crate) accept: Vec<Glob>,
    pub(crate) reject: Vec<Glob>,
    pub(crate) aliases: Aliases,
}

impl Fontconfig {
    pub(crate) fn load(
        file: &Path,
        environment: &Environment,
        problems: &mut Vec<LoadProblem>,
    ) -> Self {
        let mut loader = Loader::new(environment, problems);
        loader.load_main(file);
        loader.finish()
    }

    pub(crate) fn accepts(&self, path: &Path) -> bool {
        let bytes = path.as_os_str().as_encoded_bytes();
        self.accept.iter().any(|glob| glob.matches(bytes))
            || !self.reject.iter().any(|glob| glob.matches(bytes))
    }
}
