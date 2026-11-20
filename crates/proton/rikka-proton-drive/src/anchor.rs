use std::fmt::Display;

use rikka_proton_api::{Page, Paging};
use serde::Deserialize;

pub(crate) const ANCHOR: &str = "AnchorID";

pub(crate) fn anchored() -> Paging {
    Paging::cursor(ANCHOR)
}

#[derive(Debug, Deserialize)]
pub(crate) struct Anchor<C = String> {
    #[serde(rename = "AnchorID", default = "Option::default")]
    id: Option<C>,
    #[serde(
        rename = "More",
        default,
        deserialize_with = "rikka_proton_api::flag::deserialize"
    )]
    more: bool,
}

impl<C> Anchor<C> {
    pub(crate) fn next(&self) -> Option<&C> {
        self.id.as_ref().filter(|_| self.more)
    }
}

impl<C: Display> Anchor<C> {
    pub(crate) fn page<T>(&self, items: Vec<T>) -> Page<T> {
        Page::new(items).next(self.next())
    }
}
