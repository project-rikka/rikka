use rikka_types::Point;
use rikka_vector::{Columns, Companions, Flatness, Flattening, MAX_FLATTENED, flatten};

use crate::outline::path::Outline;

#[derive(Clone, Debug, Default)]
pub(crate) struct Flattened {
    pub(crate) size_class: i32,
    pub(crate) xs: Vec<f32>,
    pub(crate) ys: Vec<f32>,
    pub(crate) miter_xs: Vec<f32>,
    pub(crate) miter_ys: Vec<f32>,
    pub(crate) ends: Vec<usize>,
}

impl Flattened {
    pub(crate) fn build(
        &mut self,
        outline: &Outline,
        miters: Option<&[Point]>,
        size_class: i32,
        flatness: f32,
        steps: &mut Vec<u16>,
    ) -> bool {
        self.size_class = size_class;
        self.miter_xs.clear();
        self.miter_ys.clear();
        let flatness = Flatness::new(flatness * (size_class as f32 * -0.5).exp2());
        let miters = miters
            .filter(|miters| miters.len() == outline.path().points().len())
            .map(|corners| Companions {
                corners,
                xs: &mut self.miter_xs,
                ys: &mut self.miter_ys,
            });
        flatten(
            outline.path(),
            Flattening {
                flatness,
                limit: MAX_FLATTENED,
                steps,
            },
            Columns {
                xs: &mut self.xs,
                ys: &mut self.ys,
                ends: &mut self.ends,
            },
            miters,
        )
    }

    #[inline]
    pub(crate) fn has_miters(&self) -> bool {
        self.miter_xs.len() == self.xs.len()
    }
}
