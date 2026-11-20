use std::sync::Arc;

use rikka_gpu::Compute;
use rikka_physics_cloth::{Cloth, Drape};

use crate::cloth_figures::ClothFigures;
use crate::error::Result;
use crate::layout::{FigureKey, Layout};

pub(crate) struct ClothScene {
    pub(crate) cloth: Option<Cloth>,
    pub(crate) figures: ClothFigures,
}

impl ClothScene {
    pub(crate) fn build(layout: &Layout, compute: &Compute) -> Result<ClothScene> {
        let mut cloth = if layout.has_cloth() {
            Some(
                layout
                    .cloth
                    .rate(layout.step_rate.per_second() as f32)
                    .build(compute)?,
            )
        } else {
            None
        };
        let mut figures = ClothFigures::default();
        if let Some(cloth) = cloth.as_mut() {
            for (index, figure) in layout.figures.iter().enumerate() {
                if let Some(asset) = &figure.asset {
                    figures.add(index, cloth.add_figure(Arc::clone(asset))?);
                }
            }
        }
        Ok(ClothScene { cloth, figures })
    }

    pub(crate) fn drapes(&self, layout: &Layout) -> Vec<(FigureKey, Vec<Drape>)> {
        let Some(cloth) = self.cloth.as_ref() else {
            return Vec::new();
        };
        self.figures
            .iter()
            .filter_map(|(index, figure)| Some((layout.key_of(index)?, cloth.drapes(figure).ok()?)))
            .collect()
    }
}
