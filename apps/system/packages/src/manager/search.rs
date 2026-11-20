use std::sync::Arc;

use nixpkgs_search::{Cancel, Catalog, Package, Result};
use rikka_app::prelude::*;

use crate::cancel_guard::CancelGuard;
use crate::listing::{Row, Rows};

#[derive(Debug)]
pub(super) struct Search {
    query: TextDocument,
    found: Rows,
    everything: Option<Rows>,
    looked_up_attributes: Vec<String>,
    looked_up: bool,
    described: Vec<Arc<Package>>,
    describing: CancelGuard,
}

impl Search {
    pub(super) fn new() -> Self {
        Self {
            query: TextDocument::new(""),
            found: Rows::default(),
            everything: None,
            looked_up_attributes: Vec::new(),
            looked_up: false,
            described: Vec::new(),
            describing: CancelGuard::default(),
        }
    }

    pub(super) fn query(&self) -> &TextDocument {
        &self.query
    }

    pub(super) fn query_text(&self) -> Option<String> {
        let text = self.query.text();
        (!text.trim().is_empty()).then_some(text)
    }

    pub(super) fn found(&self) -> &Rows {
        &self.found
    }

    pub(super) fn show_all(&mut self, catalog: &Catalog) {
        let everything = self
            .everything
            .get_or_insert_with(|| Arc::new(catalog.ids().map(Row::Cataloged).collect()));
        self.found = Arc::clone(everything);
    }

    pub(super) fn reset(&mut self) {
        self.everything = None;
        self.found = Rows::default();
    }

    pub(super) fn set_found(&mut self, found: Rows) {
        self.found = found;
    }

    pub(super) fn looked_up(&self) -> bool {
        self.looked_up
    }

    pub(super) fn describe(&mut self, listed: Vec<String>) -> Option<Cancel> {
        if listed.is_empty() || listed == self.looked_up_attributes {
            return None;
        }
        self.looked_up_attributes = listed;
        Some(self.describing.renew())
    }

    pub(super) fn described(&mut self, described: Result<Vec<Package>>) -> Result<()> {
        self.looked_up = true;
        match described {
            Ok(packages) => {
                self.described = packages.into_iter().map(Arc::new).collect();
                Ok(())
            }
            Err(error) => {
                self.looked_up_attributes.clear();
                Err(error)
            }
        }
    }

    pub(super) fn described_attributes(&self) -> Vec<String> {
        self.described
            .iter()
            .map(|found| found.attribute.clone())
            .collect()
    }

    pub(super) fn description(&self, attribute: &str) -> Option<&Arc<Package>> {
        self.described
            .iter()
            .find(|found| found.attribute == attribute)
    }
}

pub(super) fn found(catalog: &Catalog, text: &str) -> Rows {
    Arc::new(
        catalog
            .search(text)
            .into_iter()
            .map(Row::Cataloged)
            .collect(),
    )
}
