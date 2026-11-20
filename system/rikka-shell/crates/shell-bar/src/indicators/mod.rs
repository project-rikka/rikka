mod indicator;
mod panel;
mod parts;
mod shelf;
mod typing;
mod usage;

use std::rc::Rc;

pub(crate) use indicator::Indicator;
pub(crate) use panel::{Drawer, view as panel};
pub(crate) use rikka_settings::session::appearance::Indicator as Kind;
pub(crate) use shelf::Shelf;
pub(crate) use typing::Typing;
pub(crate) use usage::Usage;

#[must_use]
pub(crate) fn all(typing: Typing, usage: Usage) -> Vec<Rc<dyn Indicator>> {
    vec![Rc::new(typing), Rc::new(usage)]
}
