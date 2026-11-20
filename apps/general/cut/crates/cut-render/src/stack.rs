use cut_project::{Clip, sampled};
use rikka_effects::Effect;
use rikka_image_edit::{GaussianBlur, Recipe, Step};

use crate::overrides::Overrides;

const PAD_STEP: u32 = 8;
const MAX_PAD: u32 = 2048;
const BLUR_REACH: f32 = 3.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Stack {
    recipe: Recipe,
    reach: f32,
}

impl Stack {
    pub(crate) fn sample(clip: &Clip, frame: i64, overrides: &Overrides) -> Stack {
        let effects = match overrides.effects(clip.id()) {
            Some(slots) => sampled(slots, clip.local(frame) as f64),
            None => clip.effects_at(frame),
        };
        Stack::from_effects(effects)
    }

    pub(crate) fn from_effects(effects: impl IntoIterator<Item = Effect>) -> Stack {
        let mut stack = Stack::default();
        for effect in effects {
            let recipe = effect.recipe();
            if recipe.is_empty() {
                continue;
            }
            for step in recipe.steps() {
                stack.recipe.push(step.clone());
            }
            stack.reach += effect.padding().max(0.0);
        }
        stack
    }

    pub(crate) fn blurred(mut self, sigma: f32) -> Stack {
        if sigma > 0.0 && sigma.is_finite() {
            self.recipe.push(Step::new(GaussianBlur::new(sigma)));
            self.reach += sigma * BLUR_REACH;
        }
        self
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.recipe.is_empty()
    }

    pub(crate) fn reach(&self) -> f32 {
        self.reach
    }

    pub(crate) fn scaled(&self, factor: f32) -> Recipe {
        match factor == 1.0 {
            true => self.recipe.clone(),
            false => self.recipe.scaled(factor),
        }
    }

    pub(crate) fn padding(&self, factor: f32) -> u32 {
        let reach = self.reach * factor;
        if reach <= 0.0 || !reach.is_finite() {
            return 0;
        }
        ((reach.ceil() as u32).div_ceil(PAD_STEP) * PAD_STEP).min(MAX_PAD)
    }
}
