use std::path::Path;
use std::sync::Arc;

use crate::build::ModelBuilder;
use crate::error::{Error, Result};
use crate::journal::Journal;
use crate::model::Model;
use crate::options::{ImportOptions, Limit};
use crate::progress::Stage;
use crate::resolver::Resolver;
use crate::texture::{Image, ImageId};
use crate::warning::Warning;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImporterInfo {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
}

impl ImporterInfo {
    #[must_use]
    pub fn reads_extension(&self, extension: &str) -> bool {
        self.extensions
            .iter()
            .any(|known| known.eq_ignore_ascii_case(extension))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confidence {
    No,
    Maybe,
    Yes,
}

#[derive(Clone, Copy, Debug)]
pub struct Input<'a> {
    bytes: &'a [u8],
    path: Option<&'a Path>,
}

impl<'a> Input<'a> {
    #[must_use]
    pub fn new(bytes: &'a [u8], path: Option<&'a Path>) -> Self {
        Self { bytes, path }
    }

    #[must_use]
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    #[must_use]
    pub fn path(&self) -> Option<&'a Path> {
        self.path
    }

    #[must_use]
    pub fn extension(&self) -> Option<&'a str> {
        self.path?.extension()?.to_str()
    }

    #[must_use]
    pub fn head(&self, length: usize) -> &'a [u8] {
        &self.bytes[..length.min(self.bytes.len())]
    }
}

pub trait Importer: Send + Sync + 'static {
    fn info(&self) -> ImporterInfo;

    fn sniff(&self, head: &[u8]) -> Confidence;

    fn read(&self, input: &Input<'_>, ctx: &mut ImportCtx<'_>) -> Result<Model>;
}

pub struct ImportCtx<'a> {
    options: &'a ImportOptions,
    resolver: &'a dyn Resolver,
    journal: Journal<'a>,
    spent: u64,
    keyframes: u64,
}

impl<'a> ImportCtx<'a> {
    pub(crate) fn new(
        options: &'a ImportOptions,
        resolver: &'a dyn Resolver,
        journal: Journal<'a>,
    ) -> Self {
        Self {
            options,
            resolver,
            journal,
            spent: 0,
            keyframes: 0,
        }
    }

    pub fn options(&self) -> &ImportOptions {
        self.options
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

    pub fn charge(&mut self, bytes: u64) -> Result<()> {
        self.spent = self.spent.saturating_add(bytes);
        self.options.limits.check(Limit::TotalBytes, self.spent)
    }

    pub fn charge_keyframes(&mut self, keyframes: usize) -> Result<()> {
        self.keyframes = self.keyframes.saturating_add(keyframes as u64);
        self.options.limits.check(Limit::Keyframes, self.keyframes)
    }

    pub fn read_resource(&mut self, uri: &str) -> Result<Arc<[u8]>> {
        self.checkpoint()?;
        let bytes = self
            .resolver
            .read(uri, self.options.limits.max_resource_bytes)?;
        self.charge(bytes.len() as u64)?;
        Ok(bytes)
    }

    pub fn read_optional_resource(&mut self, uri: &str) -> Result<Option<Arc<[u8]>>> {
        match self.read_resource(uri) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(Error::Read { uri, reason }) => {
                self.warn(Warning::MissingResource { uri, reason });
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    pub fn import_image(
        &mut self,
        builder: &mut ModelBuilder,
        places: &[&str],
    ) -> Result<Option<ImageId>> {
        let Some((&name, earlier)) = places.split_last() else {
            return Ok(None);
        };
        for &uri in earlier.iter().filter(|&&uri| uri != name) {
            match self.read_resource(uri) {
                Ok(bytes) => return Ok(Some(builder.add_image(Image::new(name, bytes)))),
                Err(Error::Read { .. }) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(self
            .read_optional_resource(name)?
            .map(|bytes| builder.add_image(Image::new(name, bytes))))
    }

    pub(crate) fn take_warnings(&mut self) -> Vec<Warning> {
        self.journal.take()
    }
}

impl std::fmt::Debug for ImportCtx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportCtx")
            .field("options", self.options)
            .field("spent", &self.spent)
            .finish_non_exhaustive()
    }
}
