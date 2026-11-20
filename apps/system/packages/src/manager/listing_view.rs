use std::sync::Arc;

use nixpkgs_search::{Package, Result};

use crate::listing::{Line, Listing};

#[derive(Debug, Default)]
pub(super) struct ListingView {
    listing: Listing,
    revision: u64,
    selected: Option<usize>,
    package: Option<Arc<Package>>,
}

impl ListingView {
    pub(super) fn listing(&self) -> &Listing {
        &self.listing
    }

    pub(super) fn revision(&self) -> u64 {
        self.revision
    }

    pub(super) fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub(super) fn selected_row(&self) -> Option<(usize, Line<'_>, Option<&Package>)> {
        let at = self.selected?;
        Some((at, self.listing.line(at)?, self.package.as_deref()))
    }

    pub(super) fn set_listing(&mut self, listing: Listing) -> Result<()> {
        if listing.has_same_rows(&self.listing) {
            self.listing = listing;
            return Ok(());
        }
        let selected = self
            .selected
            .and_then(|at| self.listing.attribute(at))
            .map(str::to_owned);
        self.listing = listing;
        self.revision = self.revision.wrapping_add(1);
        self.selected = selected.and_then(|attribute| self.listing.position(&attribute));
        match self.selected {
            Some(at) => self.package = self.listing.package(at)?,
            None => self.package = None,
        }
        Ok(())
    }

    pub(super) fn select(&mut self, at: usize) -> Result<bool> {
        if at >= self.listing.len() || self.selected == Some(at) {
            return Ok(false);
        }
        self.selected = Some(at);
        self.package = None;
        self.package = self.listing.package(at)?;
        Ok(true)
    }

    pub(super) fn deselect(&mut self) {
        self.selected = None;
        self.package = None;
    }
}
