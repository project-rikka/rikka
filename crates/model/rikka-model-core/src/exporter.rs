use std::path::Path;

use crate::error::Result;
use crate::journal::Journal;
use crate::model::Model;
use crate::options::{ExportOptions, Limit};
use crate::progress::Stage;
use crate::sink::Sink;
use crate::warning::Warning;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[must_use]
pub struct Supports {
    pub skins: bool,
    pub animations: bool,
    pub morph_targets: bool,
    pub cameras: bool,
    pub lights: bool,
}

impl Supports {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn all() -> Self {
        Self {
            skins: true,
            animations: true,
            morph_targets: true,
            cameras: true,
            lights: true,
        }
    }

    pub fn skins(mut self, supported: bool) -> Self {
        self.skins = supported;
        self
    }

    pub fn animations(mut self, supported: bool) -> Self {
        self.animations = supported;
        self
    }

    pub fn morph_targets(mut self, supported: bool) -> Self {
        self.morph_targets = supported;
        self
    }

    pub fn cameras(mut self, supported: bool) -> Self {
        self.cameras = supported;
        self
    }

    pub fn lights(mut self, supported: bool) -> Self {
        self.lights = supported;
        self
    }

    pub(crate) fn dropped_from(&self, model: &Model) -> Vec<Warning> {
        let checks = [
            (!self.skins && !model.skins.is_empty(), "skins"),
            (
                !self.animations && !model.animations.is_empty(),
                "animations",
            ),
            (
                !self.morph_targets
                    && model
                        .meshes
                        .iter()
                        .any(|mesh| !mesh.morph_targets.is_empty()),
                "morph targets",
            ),
            (!self.cameras && !model.cameras.is_empty(), "cameras"),
            (!self.lights && !model.lights.is_empty(), "lights"),
        ];
        checks
            .into_iter()
            .filter(|(dropped, _)| *dropped)
            .map(|(_, what)| {
                Warning::Unsupported(format!("format cannot store {what}, dropping them"))
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExporterInfo {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub supports: Supports,
}

impl ExporterInfo {
    #[must_use]
    pub fn writes_extension(&self, extension: &str) -> bool {
        self.extensions
            .iter()
            .any(|known| known.eq_ignore_ascii_case(extension))
    }
}

pub trait Exporter: Send + Sync + 'static {
    fn info(&self) -> ExporterInfo;

    fn write(&self, model: &Model, ctx: &mut ExportCtx<'_>) -> Result<Vec<u8>>;
}

pub struct ExportCtx<'a> {
    options: &'a ExportOptions,
    sink: &'a dyn Sink,
    journal: Journal<'a>,
    name: &'a Path,
    written: u64,
}

impl<'a> ExportCtx<'a> {
    pub(crate) fn new(
        options: &'a ExportOptions,
        sink: &'a dyn Sink,
        journal: Journal<'a>,
        name: &'a Path,
    ) -> Self {
        Self {
            options,
            sink,
            journal,
            name,
            written: 0,
        }
    }

    pub fn options(&self) -> &ExportOptions {
        self.options
    }

    #[must_use]
    pub fn name(&self) -> &Path {
        self.name
    }

    #[must_use]
    pub fn embeds(&self) -> bool {
        self.options.embed_resources
    }

    pub fn checkpoint(&self) -> Result<()> {
        self.journal.checkpoint()
    }

    pub fn report(&self, stage: Stage, fraction: f32) {
        self.journal.report(stage, fraction);
    }

    pub fn warn(&mut self, warning: Warning) {
        self.journal.warn(warning);
    }

    pub fn write_resource(&mut self, uri: &str, bytes: &[u8]) -> Result<()> {
        self.checkpoint()?;
        self.written = self.written.saturating_add(bytes.len() as u64);
        self.options.limits.check(Limit::TotalBytes, self.written)?;
        self.sink.write(uri, bytes)
    }

    pub(crate) fn hold(&mut self, warnings: Vec<Warning>) {
        self.journal.hold(warnings);
    }

    pub(crate) fn take_warnings(&mut self) -> Vec<Warning> {
        self.journal.take()
    }
}

impl std::fmt::Debug for ExportCtx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportCtx")
            .field("options", self.options)
            .field("name", &self.name)
            .field("written", &self.written)
            .finish_non_exhaustive()
    }
}
