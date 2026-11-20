mod blend;
pub(crate) mod develop;
pub(crate) mod mask;
mod preview;

use crate::ops::{Operation, Reach};

pub use blend::Blend;
pub use mask::{Mask, Span};
pub use preview::Preview;

#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    operation: Operation,
    mask: Option<Mask>,
    opacity: f32,
    blend: Blend,
}

impl Step {
    #[must_use]
    pub fn new(operation: impl Into<Operation>) -> Step {
        Step {
            operation: operation.into(),
            mask: None,
            opacity: 1.0,
            blend: Blend::Normal,
        }
    }

    #[must_use]
    pub fn masked(mut self, mask: Mask) -> Step {
        self.mask = Some(mask);
        self
    }

    #[must_use]
    pub fn opacity(mut self, opacity: f32) -> Step {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    #[must_use]
    pub fn blend(mut self, blend: Blend) -> Step {
        self.blend = blend;
        self
    }

    #[must_use]
    pub fn operation(&self) -> &Operation {
        &self.operation
    }

    #[must_use]
    pub fn scaled(&self, factor: f32) -> Step {
        Step {
            operation: self.operation.scaled(factor),
            mask: self.mask.as_ref().map(|mask| mask.scaled(factor)),
            ..self.clone()
        }
    }

    pub(crate) fn reach(&self, canvas: [u32; 2]) -> Reach {
        let reach = self.operation.reach(canvas);
        match &self.mask {
            Some(mask) => reach.intersect(mask.reach(canvas)),
            None => reach,
        }
    }

    pub(crate) fn is_plain(&self) -> bool {
        self.mask.is_none() && self.opacity >= 1.0 && self.blend == Blend::Normal
    }
}

impl From<Operation> for Step {
    fn from(operation: Operation) -> Step {
        Step::new(operation)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recipe {
    steps: Vec<Step>,
}

impl Recipe {
    #[must_use]
    pub fn new() -> Recipe {
        Recipe::default()
    }

    #[must_use]
    pub fn then(mut self, step: impl Into<Step>) -> Recipe {
        self.steps.push(step.into());
        self
    }

    pub fn push(&mut self, step: impl Into<Step>) {
        self.steps.push(step.into());
    }

    pub fn remove(&mut self, at: usize) -> Option<Step> {
        (at < self.steps.len()).then(|| self.steps.remove(at))
    }

    pub fn replace(&mut self, at: usize, step: impl Into<Step>) -> bool {
        match self.steps.get_mut(at) {
            Some(slot) => {
                *slot = step.into();
                true
            }
            None => false,
        }
    }

    #[must_use]
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    #[must_use]
    pub fn scaled(&self, factor: f32) -> Recipe {
        self.steps.iter().map(|step| step.scaled(factor)).collect()
    }

    pub(crate) fn reach(&self, canvas: [u32; 2]) -> Reach {
        self.steps.iter().fold(Reach::Nowhere, |reach, step| {
            reach.union(step.reach(canvas))
        })
    }
}

impl<S: Into<Step>> FromIterator<S> for Recipe {
    fn from_iter<I: IntoIterator<Item = S>>(steps: I) -> Recipe {
        Recipe {
            steps: steps.into_iter().map(Into::into).collect(),
        }
    }
}
