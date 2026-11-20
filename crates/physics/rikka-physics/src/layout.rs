use std::sync::Arc;

use rikka_model_core::Model;
use rikka_physics_cloth::{Cloth, ClothBuilder};
use rikka_physics_fluid::FluidBuilder;
use rikka_physics_tailor::ClothAsset;

use crate::step_rate::StepRate;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FigureKey(pub u64);

#[derive(Clone, Debug)]
pub(crate) struct Figure {
    pub(crate) key: FigureKey,
    pub(crate) model: Arc<Model>,
    pub(crate) asset: Option<Arc<ClothAsset>>,
}

impl PartialEq for Figure {
    fn eq(&self, other: &Figure) -> bool {
        let same_asset = match (&self.asset, &other.asset) {
            (Some(asset), Some(other)) => Arc::ptr_eq(asset, other),
            (None, None) => true,
            _ => false,
        };
        self.key == other.key && Arc::ptr_eq(&self.model, &other.model) && same_asset
    }
}

#[must_use]
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub(crate) fluid: FluidBuilder,
    pub(crate) cloth: ClothBuilder,
    pub(crate) drying: f32,
    pub(crate) step_rate: StepRate,
    pub(crate) figures: Vec<Figure>,
}

impl Layout {
    pub fn new(fluid: FluidBuilder) -> Layout {
        Layout {
            fluid,
            cloth: Cloth::builder(),
            drying: 0.0,
            step_rate: StepRate::default(),
            figures: Vec::new(),
        }
    }

    pub fn cloth(mut self, cloth: ClothBuilder) -> Layout {
        self.cloth = cloth;
        self
    }

    pub fn drying(mut self, per_second: f32) -> Layout {
        self.drying = per_second;
        self
    }

    pub fn step_rate(mut self, step_rate: StepRate) -> Layout {
        self.step_rate = step_rate;
        self
    }

    pub fn figure(mut self, key: FigureKey, model: Arc<Model>) -> Layout {
        self.figures.push(Figure {
            key,
            model,
            asset: None,
        });
        self
    }

    pub fn figure_with_cloth(
        mut self,
        key: FigureKey,
        model: Arc<Model>,
        asset: Arc<ClothAsset>,
    ) -> Layout {
        self.figures.push(Figure {
            key,
            model,
            asset: Some(asset),
        });
        self
    }

    pub(crate) fn key_of(&self, figure: usize) -> Option<FigureKey> {
        self.figures.get(figure).map(|figure| figure.key)
    }

    pub(crate) fn has_cloth(&self) -> bool {
        self.figures.iter().any(|figure| figure.asset.is_some())
    }
}
