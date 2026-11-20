use super::cache::KeyCollector;
use super::hull_view::HullStorage;
use super::pipeline::{Settings, Work};
use crate::{Manifold, Tolerance};

#[derive(Debug, Default)]
pub struct MeshContactScratch {
    pub(crate) tolerance: Tolerance,
    pub(crate) collector: KeyCollector,
    pub(crate) work: Work,
    pub(crate) hull: HullStorage,
    pub(crate) manifolds: Vec<Manifold>,
}

impl MeshContactScratch {
    pub fn new(tolerance: Tolerance) -> Self {
        Self {
            tolerance,
            ..Self::default()
        }
    }

    #[inline]
    pub(crate) fn settings(&self, speculative: f32) -> Settings {
        let linear_slop = self.tolerance.linear_slop();
        Settings {
            speculative,
            linear_slop,
            rest_offset: linear_slop,
        }
    }

    #[inline]
    pub(crate) fn margin(&self, speculative: f32) -> f32 {
        0.05 * self.tolerance.length_scale + speculative
    }
}
