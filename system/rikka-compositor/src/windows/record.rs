use std::time::Instant;

use rikka_compositor_shell::{ForeignToplevelId, ToplevelConfigure};

use super::decoration::Decorated;
use super::mode::Mode;
use super::stretch::Stretch;

#[derive(Debug, Default)]
pub(crate) struct Record {
    pub(crate) configure: ToplevelConfigure,
    pub(crate) configured_at: Option<Instant>,
    pub(crate) mode: Mode,
    pub(crate) floated: bool,
    pub(crate) foreign: Option<ForeignToplevelId>,
    pub(crate) stretch: Option<Stretch>,
    pub(crate) decorated: Option<Decorated>,
}
