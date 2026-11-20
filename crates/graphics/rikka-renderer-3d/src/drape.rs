use crate::compute_inputs::{ComputeSource, PerVertex};
use crate::error::Result;

pub const DRAPE_VERTEX_BYTES: u64 = 48;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Drape {
    pub(crate) source: u32,
    pub(crate) first: u32,
}

impl Drape {
    #[must_use]
    pub fn with_first_vertex(self, first_vertex: u32) -> Self {
        Self {
            first: self.first.saturating_add(first_vertex),
            ..self
        }
    }
}

pub(crate) fn drape_address(sources: &[ComputeSource], drape: Drape, vertices: u32) -> Result<u64> {
    PerVertex {
        what: "drape",
        sources,
        stride: DRAPE_VERTEX_BYTES,
    }
    .address(drape.source, drape.first, vertices)
}
