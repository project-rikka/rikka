use rikka_types::Color;

use crate::error::{Error, Result};
use crate::ops::Effect;
use crate::ops::geometry::Fill;
use crate::ops::passes::composite_over;
use crate::picture::Frame;
use crate::recipe::Recipe;
use crate::recipe::develop::apply;
use crate::work::Work;

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    recipe: Recipe,
    scale: f32,
}

impl Group {
    #[must_use]
    pub fn new(recipe: Recipe) -> Group {
        Group { recipe, scale: 1.0 }
    }

    #[must_use]
    pub fn recipe(&self) -> &Recipe {
        &self.recipe
    }
}

impl Effect for Group {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        if self.recipe.is_empty() {
            return Ok(frame.clone());
        }
        let mut layer = Fill::new(Color::TRANSPARENT).apply(work, frame)?;
        for (index, step) in self.recipe.steps().iter().enumerate() {
            layer = apply(work, &layer, step, self.scale, index as u32)?;
            if layer.extent() != frame.extent() {
                return Err(Error::Invalid(
                    "group cannot contain reshaping steps".into(),
                ));
            }
        }
        let out = composite_over(work, frame, &layer.plane, [0, 0])?;
        Ok(frame.with_plane(out))
    }

    fn scaled(&self, factor: f32) -> Group {
        Group {
            scale: self.scale * factor,
            ..self.clone()
        }
    }
}
