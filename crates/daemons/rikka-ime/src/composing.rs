use rikka_types::Rect;

use crate::candidate::Candidates;
use crate::composition::Preedit;
use crate::property::Property;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Composing {
    pub preedit: Preedit,
    pub aux: String,
    pub candidates: Candidates,
    pub spot: Option<Rect>,
    pub status: Option<Property>,
}

impl Composing {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.preedit.is_empty() && self.aux.is_empty() && self.candidates.is_empty()
    }
}
