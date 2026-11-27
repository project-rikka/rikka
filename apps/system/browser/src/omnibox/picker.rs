use browser_store::Store;
use rikka_app::prelude::*;

use crate::omnibox::listing::Listing;
use crate::omnibox::lookup::Lookup;

#[derive(State)]
pub(crate) struct Picker {
    #[untracked]
    pub(crate) lookup: Option<Lookup>,
    #[untracked]
    pub(crate) arrowed: bool,
    #[untracked]
    pub(crate) deleting: bool,
    #[untracked]
    pub(crate) store: Option<Store>,
    listing: Tracked<Option<Listing>>,
}

impl Picker {
    pub(crate) fn new() -> Picker {
        Picker {
            lookup: None,
            arrowed: false,
            deleting: false,
            store: None,
            listing: Tracked::new(None),
        }
    }

    pub(crate) fn listing(&self) -> Option<&Listing> {
        self.listing.as_ref()
    }

    pub(crate) fn listing_mut(&mut self) -> Option<&mut Listing> {
        self.listing.as_mut()
    }

    pub(crate) fn show(&mut self, listing: Option<Listing>) {
        self.listing.set(listing);
    }

    pub(crate) fn hide(&mut self) {
        self.show(None);
        self.arrowed = false;
    }

    pub(crate) fn reset(&mut self) {
        self.hide();
        self.lookup = None;
        self.deleting = false;
    }

    pub(crate) fn selected(&self) -> Option<usize> {
        self.listing()?.selected
    }
}

impl std::fmt::Debug for Picker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Picker")
            .field("arrowed", &self.arrowed)
            .finish_non_exhaustive()
    }
}
