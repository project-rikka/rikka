use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_types::CancelToken;

use crate::coordinates::Conversion;
use crate::error::{Error, Result};
use crate::exporter::{ExportCtx, Exporter};
use crate::journal::{Journal, Report};
use crate::model::Model;
use crate::options::ExportOptions;
use crate::postprocess::convert_coordinates;
use crate::progress::{Progress, Stage};
use crate::registry::Registry;
use crate::sink::{DirectorySink, NoSink, Sink};
use crate::warning::Warning;

#[derive(Debug)]
pub struct Exported {
    pub bytes: Vec<u8>,
    pub warnings: Vec<Warning>,
}

#[must_use]
pub struct Export<'a> {
    model: &'a Model,
    choice: ExporterChoice,
    sink: Option<Arc<dyn Sink>>,
    options: ExportOptions,
    cancel: CancelToken,
    progress: Option<Report>,
}

enum ExporterChoice {
    Registry(Arc<Registry>),
    Exporter(Arc<dyn Exporter>),
}

impl std::fmt::Debug for ExporterChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExporterChoice::Registry(registry) => registry.fmt(f),
            ExporterChoice::Exporter(exporter) => f.write_str(exporter.info().name),
        }
    }
}

impl<'a> Export<'a> {
    pub fn new(model: &'a Model, registry: Arc<Registry>) -> Self {
        Self::with_choice(model, ExporterChoice::Registry(registry))
    }

    pub fn with_exporter(model: &'a Model, exporter: Arc<dyn Exporter>) -> Self {
        Self::with_choice(model, ExporterChoice::Exporter(exporter))
    }

    fn with_choice(model: &'a Model, choice: ExporterChoice) -> Self {
        Self {
            model,
            choice,
            sink: None,
            options: ExportOptions::default(),
            cancel: CancelToken::new(),
            progress: None,
        }
    }

    pub fn sink(mut self, sink: Arc<dyn Sink>) -> Self {
        self.sink = Some(sink);
        self
    }

    pub fn options(mut self, options: ExportOptions) -> Self {
        self.options = options;
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

    pub fn to_bytes(self, name: impl AsRef<Path>) -> Result<Exported> {
        let sink = self
            .sink
            .clone()
            .unwrap_or_else(|| Arc::new(NoSink) as Arc<dyn Sink>);
        self.write(name.as_ref(), sink)
    }

    pub fn to_file(self, path: impl Into<PathBuf>) -> Result<Vec<Warning>> {
        let path = path.into();
        let sink = self.sink.clone().unwrap_or_else(|| sibling_sink(&path));
        let exported = self.write(&path, sink)?;
        rikka_fs::replace(&path, &exported.bytes)?;
        Ok(exported.warnings)
    }

    fn write(self, name: &Path, sink: Arc<dyn Sink>) -> Result<Exported> {
        let Self {
            model,
            choice,
            options,
            cancel,
            progress,
            ..
        } = self;

        let journal = Journal::new(&cancel, progress.as_deref());
        let exporter = match &choice {
            ExporterChoice::Exporter(exporter) => exporter.as_ref(),
            ExporterChoice::Registry(registry) => registry
                .exporter_for(name)
                .ok_or_else(|| Error::UnknownFormat(name.display().to_string()))?,
        };

        model.validate()?;
        journal.checkpoint()?;

        journal.report(Stage::Converting, 0.0);
        let written = converted(model, &options);
        journal.report(Stage::Converting, 1.0);

        let mut ctx = ExportCtx::new(&options, sink.as_ref(), journal, name);
        ctx.hold(exporter.info().supports.dropped_from(&written));
        ctx.report(Stage::Writing, 0.0);
        let bytes = exporter.write(&written, &mut ctx)?;
        let warnings = ctx.take_warnings();
        ctx.report(Stage::Writing, 1.0);

        Ok(Exported { bytes, warnings })
    }
}

impl std::fmt::Debug for Export<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Export")
            .field("model", &self.model.metadata.source)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

fn converted<'a>(model: &'a Model, options: &ExportOptions) -> Cow<'a, Model> {
    let scale = model.metadata.unit_scale / options.unit_scale.max(f32::MIN_POSITIVE);
    let conversion = Conversion::new(model.metadata.coordinates, options.coordinates, scale);
    if conversion.is_identity() {
        return Cow::Borrowed(model);
    }
    let mut copy = model.clone();
    convert_coordinates(&mut copy, options.coordinates, scale);
    copy.metadata.unit_scale = options.unit_scale;
    Cow::Owned(copy)
}

fn sibling_sink(path: &Path) -> Arc<dyn Sink> {
    match path.parent() {
        Some(folder) if !folder.as_os_str().is_empty() => Arc::new(DirectorySink::new(folder)),
        _ => Arc::new(DirectorySink::new(".")),
    }
}
