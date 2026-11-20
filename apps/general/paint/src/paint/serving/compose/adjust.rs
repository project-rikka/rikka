use paint_assistant::AdjustStep;
use paint_document::{Edit, LayerId};
use rikka_image_edit::Recipe;

use super::{Composer, Refusal, refused, stepped};

impl Composer<'_> {
    pub(super) fn adjust(&mut self, steps: Vec<AdjustStep>) -> Result<(), Refusal> {
        let (width, height) = self.size();
        let selection = self.selection_mask()?;
        let mut planned: Vec<(LayerId, Recipe)> = Vec::new();
        for (at, step) in steps.into_iter().enumerate() {
            let layer = stepped("step", at, "adjust", self.paintable(step.layer.as_ref()))?;
            let mask = match &step.mask {
                Some(mask) => Some(stepped(
                    "step",
                    at,
                    "mask",
                    mask.mask(width, height).map_err(refused),
                )?),
                None => selection.clone(),
            };
            let developed = step.recipe.steps().iter().map(|developed| match &mask {
                Some(mask) => developed.clone().masked(mask.clone()),
                None => developed.clone(),
            });
            match planned.last_mut() {
                Some((last, recipe)) if *last == layer => {
                    developed.for_each(|step| recipe.push(step))
                }
                _ => planned.push((layer, developed.fold(Recipe::new(), Recipe::then))),
            }
        }
        for (layer, recipe) in planned {
            self.apply(&Edit::Develop { layer, recipe })?;
        }
        Ok(())
    }
}
