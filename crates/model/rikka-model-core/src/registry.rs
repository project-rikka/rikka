use std::path::Path;
use std::sync::Arc;

use crate::exporter::Exporter;
use crate::importer::{Confidence, Importer, Input};

#[derive(Clone, Default)]
pub struct Registry {
    importers: Vec<Arc<dyn Importer>>,
    exporters: Vec<Arc<dyn Exporter>>,
}

impl Registry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_importer(mut self, importer: impl Importer) -> Self {
        self.importers.push(Arc::new(importer));
        self
    }

    #[must_use]
    pub fn with_exporter(mut self, exporter: impl Exporter) -> Self {
        self.exporters.push(Arc::new(exporter));
        self
    }

    pub fn importers(&self) -> impl ExactSizeIterator<Item = &dyn Importer> {
        self.importers.iter().map(AsRef::as_ref)
    }

    pub fn exporters(&self) -> impl ExactSizeIterator<Item = &dyn Exporter> {
        self.exporters.iter().map(AsRef::as_ref)
    }

    #[must_use]
    pub fn import_extensions(&self) -> Vec<&'static str> {
        sorted(
            self.importers
                .iter()
                .flat_map(|importer| importer.info().extensions.iter().copied()),
        )
    }

    #[must_use]
    pub fn export_extensions(&self) -> Vec<&'static str> {
        sorted(
            self.exporters
                .iter()
                .flat_map(|exporter| exporter.info().extensions.iter().copied()),
        )
    }

    #[must_use]
    pub fn exporter_for(&self, name: &Path) -> Option<&dyn Exporter> {
        let extension = name.extension()?.to_str()?;
        self.exporters()
            .find(|exporter| exporter.info().writes_extension(extension))
    }

    #[must_use]
    pub fn exporter_named(&self, name: &str) -> Option<&dyn Exporter> {
        self.exporters()
            .find(|exporter| exporter.info().name == name)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.importers.is_empty() && self.exporters.is_empty()
    }

    #[must_use]
    pub fn importer_for(&self, input: &Input<'_>) -> Option<&dyn Importer> {
        let head = input.head(HEAD);
        let extension = input.extension();
        let sniffed: Vec<(&dyn Importer, Confidence)> = self
            .importers()
            .map(|importer| (importer, importer.sniff(head)))
            .collect();

        let best = sniffed
            .iter()
            .map(|(_, confidence)| *confidence)
            .max()
            .filter(|confidence| *confidence != Confidence::No)?;
        let sure = sniffed.iter().filter(|(_, confidence)| *confidence == best);

        let mut first_sure = None;
        for (importer, _) in sure {
            if extension.is_some_and(|extension| importer.info().reads_extension(extension)) {
                return Some(*importer);
            }
            first_sure = first_sure.or(Some(*importer));
        }
        first_sure
    }
}

const HEAD: usize = 4096;

fn sorted(names: impl Iterator<Item = &'static str>) -> Vec<&'static str> {
    let mut names: Vec<&'static str> = names.collect();
    names.sort_unstable();
    names.dedup();
    names
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field(
                "importers",
                &self
                    .importers()
                    .map(|it| it.info().name)
                    .collect::<Vec<_>>(),
            )
            .field(
                "exporters",
                &self
                    .exporters()
                    .map(|it| it.info().name)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}
