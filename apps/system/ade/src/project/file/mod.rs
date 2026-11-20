mod pane;
pub(in crate::project) mod state;
mod strip;

pub(crate) use state::Viewer;

pub(super) use pane::view as page;
