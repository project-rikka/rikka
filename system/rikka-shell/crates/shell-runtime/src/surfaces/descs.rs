use std::ops::{Index, IndexMut};

use shell_kit::hosting::ErasedSurface;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SurfaceId(usize);

impl SurfaceId {}

#[derive(Debug)]
pub(crate) struct Descs(Vec<ErasedSurface>);

impl Descs {
    pub(crate) fn new(descs: Vec<ErasedSurface>) -> Descs {
        Descs(descs)
    }

    pub(super) fn ids(&self) -> impl Iterator<Item = SurfaceId> + use<> {
        (0..self.0.len()).map(SurfaceId)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (SurfaceId, &ErasedSurface)> {
        self.0
            .iter()
            .enumerate()
            .map(|(index, desc)| (SurfaceId(index), desc))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Index<SurfaceId> for Descs {
    type Output = ErasedSurface;

    fn index(&self, id: SurfaceId) -> &ErasedSurface {
        &self.0[id.0]
    }
}

impl IndexMut<SurfaceId> for Descs {
    fn index_mut(&mut self, id: SurfaceId) -> &mut ErasedSurface {
        &mut self.0[id.0]
    }
}
