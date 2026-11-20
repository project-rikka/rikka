use crate::compute_inputs::{ComputeSource, PerVertex};
use crate::error::Result;
use crate::fluid::gpu::WET_BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Wetness {
    pub(crate) source: u32,
    pub(crate) first: u32,
}

impl Wetness {
    #[must_use]
    pub fn with_first_vertex(self, first_vertex: u32) -> Self {
        Self {
            first: self.first.saturating_add(first_vertex),
            ..self
        }
    }
}

pub(crate) fn wet_address(
    sources: &[ComputeSource],
    wetness: Wetness,
    vertices: u32,
) -> Result<u64> {
    PerVertex {
        what: "wetness",
        sources,
        stride: WET_BYTES,
    }
    .address(wetness.source, wetness.first, vertices)
}
