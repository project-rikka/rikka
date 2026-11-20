use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_types::CancelToken;

use crate::error::{Error, Result};
use crate::importer::{ImportCtx, Input};
use crate::journal::{Journal, Report};
use crate::model::Model;
use crate::options::{ImportOptions, Limit};
use crate::postprocess;
use crate::progress::{Progress, Stage};
use crate::registry::Registry;
use crate::resolver::{DirectoryResolver, NoResolver, Resolver};

#[derive(Debug)]
enum Source {
    Path(PathBuf),
    Bytes(Arc<[u8]>, Option<PathBuf>),
}

#[must_use]
pub struct Import {
    source: Source,
    registry: Arc<Registry>,
    resolver: Option<Arc<dyn Resolver>>,
    options: ImportOptions,
    cancel: CancelToken,
    progress: Option<Report>,
}

impl Import {
    pub fn open(path: impl Into<PathBuf>, registry: Arc<Registry>) -> Self {
        Self::with_source(Source::Path(path.into()), registry)
    }

    pub fn bytes(bytes: impl Into<Arc<[u8]>>, registry: Arc<Registry>) -> Self {
        Self::with_source(Source::Bytes(bytes.into(), None), registry)
    }

    pub fn named_bytes(
        name: impl Into<PathBuf>,
        bytes: impl Into<Arc<[u8]>>,
        registry: Arc<Registry>,
    ) -> Self {
        Self::with_source(Source::Bytes(bytes.into(), Some(name.into())), registry)
    }

    fn with_source(source: Source, registry: Arc<Registry>) -> Self {
        Self {
            source,
            registry,
            resolver: None,
            options: ImportOptions::default(),
            cancel: CancelToken::new(),
            progress: None,
        }
    }

    pub fn options(mut self, options: ImportOptions) -> Self {
        self.options = options;
        self
    }

    pub fn resolver(mut self, resolver: Arc<dyn Resolver>) -> Self {
        self.resolver = Some(resolver);
        self
    }

    pub fn cancel(mut self, cancel: CancelToken) -> Self {
        self.cancel = cancel;
        self
    }

    pub fn progress(mut self, progress: impl Fn(Progress) + Send + Sync + 'static) -> Self {
        self.progress = Some(Arc::new(progress));
        self
    }

    pub fn run(self) -> Result<Model> {
        let Self {
            source,
            registry,
            resolver,
            options,
            cancel,
            progress,
        } = self;

        let journal = Journal::new(&cancel, progress.as_deref());
        journal.report(Stage::Reading, 0.0);
        let read_bytes;
        let given_bytes;
        let (bytes, path): (&[u8], Option<PathBuf>) = match source {
            Source::Path(path) => {
                read_bytes = read(&path, options.limits.max_file_bytes)?;
                (&read_bytes[..], Some(path))
            }
            Source::Bytes(bytes, name) => {
                given_bytes = bytes;
                (&given_bytes[..], name)
            }
        };
        journal.report(Stage::Reading, 1.0);
        journal.checkpoint()?;

        let input = Input::new(bytes, path.as_deref());
        let importer = registry
            .importer_for(&input)
            .ok_or_else(|| unreadable(&path))?;
        let resolver = resolver.unwrap_or_else(|| sibling_resolver(path.as_deref()));

        let mut ctx = ImportCtx::new(&options, resolver.as_ref(), journal);
        ctx.report(Stage::Parsing, 0.0);
        let mut model = importer.read(&input, &mut ctx)?;
        model.warnings.extend(ctx.take_warnings());

        if model.metadata.importer.is_empty() {
            model.metadata.importer = importer.info().name;
        }
        if model.metadata.source.is_none() {
            model.metadata.source = path;
        }

        ctx.report(Stage::Finishing, 0.0);
        postprocess::finish(&mut model, &options)?;
        ctx.report(Stage::Finishing, 1.0);
        Ok(model)
    }
}

impl std::fmt::Debug for Import {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Import")
            .field("source", &self.source)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    Limit::FileBytes.check(bytes.len() as u64, limit)?;
    Ok(bytes)
}

fn sibling_resolver(path: Option<&Path>) -> Arc<dyn Resolver> {
    match path.and_then(Path::parent) {
        Some(folder) => Arc::new(DirectoryResolver::new(folder)),
        None => Arc::new(NoResolver),
    }
}

fn unreadable(path: &Option<PathBuf>) -> Error {
    Error::UnknownFormat(match path {
        Some(path) => path.display().to_string(),
        None => "these bytes".to_owned(),
    })
}
