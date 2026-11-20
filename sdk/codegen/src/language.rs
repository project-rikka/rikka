use crate::diagnostic::Diagnostic;
use crate::languages::{
    c::C, cpp::Cpp, csharp::CSharp, go::Go, java::Java, python::Python, rust::Rust,
};
use crate::model::Api;

const LANGUAGES: &[&dyn Language] = &[&C, &Cpp, &Rust, &CSharp, &Go, &Java, &Python];

pub(crate) struct Output {
    pub(crate) path: String,
    pub(crate) contents: String,
}

impl Output {
    pub(crate) fn render(path: String, file: &impl askama::Template) -> Result<Self, Error> {
        let mut contents = file.render()?;
        contents.push('\n');
        Ok(Self { path, contents })
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("{}", .0.message)]
    Invalid(Diagnostic),
    #[error("failed to render a template: {0}")]
    Template(#[from] askama::Error),
}

impl From<Diagnostic> for Error {
    fn from(diagnostic: Diagnostic) -> Self {
        Self::Invalid(diagnostic)
    }
}

pub(crate) trait Language: Sync {
    fn name(&self) -> &'static str;

    fn generate(&self, api: &Api) -> Result<Vec<Output>, Error>;
}

pub(crate) fn find(name: &str) -> Option<&'static dyn Language> {
    LANGUAGES
        .iter()
        .copied()
        .find(|language| language.name() == name)
}

pub(crate) fn names() -> String {
    LANGUAGES
        .iter()
        .map(|language| language.name())
        .collect::<Vec<_>>()
        .join(", ")
}
