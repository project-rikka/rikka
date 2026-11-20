use crate::listing::Listing;

#[derive(Clone, Debug, PartialEq)]
pub struct Checkup {
    listing: Listing,
    answered: Option<String>,
}

impl Checkup {
    pub(crate) fn new(listing: Listing, answered: Option<String>) -> Checkup {
        Checkup { listing, answered }
    }

    #[must_use]
    pub fn listing(&self) -> &Listing {
        &self.listing
    }

    #[must_use]
    pub fn answered(&self) -> Option<&str> {
        self.answered.as_deref()
    }
}
