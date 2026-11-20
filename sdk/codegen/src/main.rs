mod abi;
mod diagnostic;
mod language;
mod languages;
mod model;
mod names;
mod syntax;

use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use language::Output;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("usage: rikka-sdk-codegen <idl> <language> <output>...\nlanguages: {names}", names = language::names())]
    Usage,
    #[error("unknown language `{0}`; expected one of {names}", names = language::names())]
    Language(String),
    #[error("failed to read {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("failed to write {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
    #[error("{0}")]
    Idl(String),
    #[error("{0}")]
    Template(askama::Error),
    #[error("the {language} binding produces `{path}`, but no output path names it")]
    Undeclared { language: String, path: String },
    #[error("the {language} binding produces nothing for {path}")]
    Missing { language: String, path: PathBuf },
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rikka-sdk-codegen: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Error> {
    let mut arguments = std::env::args_os().skip(1);
    let (Some(idl), Some(name)) = (arguments.next(), arguments.next()) else {
        return Err(Error::Usage);
    };
    let declared: Vec<PathBuf> = arguments.map(PathBuf::from).collect();
    if declared.is_empty() {
        return Err(Error::Usage);
    }
    let name = name.to_string_lossy().into_owned();
    let language = language::find(&name).ok_or_else(|| Error::Language(name.clone()))?;
    let idl = PathBuf::from(idl);
    let source = std::fs::read_to_string(&idl).map_err(|source| Error::Read {
        path: idl.clone(),
        source,
    })?;
    let located = |diagnostic: diagnostic::Diagnostic| Error::Idl(diagnostic.render(&idl, &source));
    let api = syntax::parse(&source)
        .and_then(model::resolve)
        .map_err(located)?;
    let outputs = language.generate(&api).map_err(|error| match error {
        language::Error::Invalid(diagnostic) => located(diagnostic),
        language::Error::Template(error) => Error::Template(error),
    })?;
    write(&name, outputs, &declared)
}

fn write(language: &str, outputs: Vec<Output>, declared: &[PathBuf]) -> Result<(), Error> {
    let mut pending: Vec<&Path> = declared.iter().map(PathBuf::as_path).collect();
    for output in outputs {
        let position = pending
            .iter()
            .position(|path| path.ends_with(&output.path))
            .ok_or_else(|| Error::Undeclared {
                language: language.to_owned(),
                path: output.path.clone(),
            })?;
        let path = pending.remove(position);
        std::fs::write(path, output.contents).map_err(|source| Error::Write {
            path: path.to_owned(),
            source,
        })?;
    }
    match pending.first() {
        Some(path) => Err(Error::Missing {
            language: language.to_owned(),
            path: path.to_path_buf(),
        }),
        None => Ok(()),
    }
}
