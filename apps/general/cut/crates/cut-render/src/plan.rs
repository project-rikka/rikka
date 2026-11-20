use rikka_image_edit::{Mask, Picture, Place, Recipe, Result, Step};

use crate::transit::Mix;

pub(crate) const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Layer {
    Place(Step),
    Filter {
        steps: Recipe,
        mask: Option<Mask>,
        opacity: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TransitionPlan {
    pub(crate) out: Option<Layer>,
    pub(crate) into: Option<Layer>,
    pub(crate) mix: Mix,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Entry {
    Layer(Layer),
    TransitionSpan(Box<TransitionPlan>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Plan {
    pub(crate) canvas: Picture,
    pub(crate) entries: Vec<Entry>,
}

impl Plan {
    pub(crate) fn render(&self) -> Result<Picture> {
        let mut canvas = self.canvas.clone();
        let mut pending = Recipe::new();
        for entry in &self.entries {
            match entry {
                Entry::Layer(Layer::Place(step)) => pending.push(step.clone()),
                Entry::Layer(Layer::Filter {
                    steps,
                    mask,
                    opacity,
                }) => {
                    let whole = mask.is_none() && *opacity >= 1.0;
                    if whole || steps.len() == 1 {
                        for step in steps.steps() {
                            pending.push(limited(step.clone(), mask, *opacity));
                        }
                        continue;
                    }
                    flush(&mut canvas, &mut pending)?;
                    let filtered = canvas.develop(steps)?;
                    pending.push(limited(
                        Step::new(Place::new(filtered, IDENTITY)),
                        mask,
                        *opacity,
                    ));
                }
                Entry::TransitionSpan(shots) => {
                    let filters = [&shots.out, &shots.into]
                        .into_iter()
                        .flatten()
                        .any(|side| matches!(side, Layer::Filter { .. }));
                    if filters {
                        flush(&mut canvas, &mut pending)?;
                    }
                    let clear = &shots.mix.clear;
                    let side = |layer: &Option<Layer>| match layer {
                        Some(layer) => alone(layer, &canvas, clear),
                        None => Ok(clear.clone()),
                    };
                    let passed = shots.mix.render(&side(&shots.out)?, &side(&shots.into)?)?;
                    pending.push(Step::new(Place::new(passed, IDENTITY)));
                }
            }
        }
        canvas.develop(&pending)
    }
}

fn flush(canvas: &mut Picture, pending: &mut Recipe) -> Result<()> {
    if !pending.is_empty() {
        *canvas = canvas.develop(pending)?;
        *pending = Recipe::new();
    }
    Ok(())
}

fn alone(layer: &Layer, canvas: &Picture, clear: &Picture) -> Result<Picture> {
    match layer {
        Layer::Place(step) => clear.develop(&Recipe::new().then(step.clone())),
        Layer::Filter {
            steps,
            mask,
            opacity,
        } => {
            let filtered = canvas.develop(steps)?;
            clear.develop(&Recipe::new().then(limited(
                Step::new(Place::new(filtered, IDENTITY)),
                mask,
                *opacity,
            )))
        }
    }
}

fn limited(step: Step, mask: &Option<Mask>, opacity: f32) -> Step {
    let step = match mask {
        Some(mask) => step.masked(mask.clone()),
        None => step,
    };
    match opacity < 1.0 {
        true => step.opacity(opacity),
        false => step,
    }
}
