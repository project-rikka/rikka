use paint_document::{AddMethod, Combine, Edit, FileFormat};
use rikka_app::prelude::*;
use rikka_image_edit::{Gradient, GradientInk, Marks, Recipe};

use super::Tab;
use crate::colors::Slot;
use crate::options::{Flooding, GradientMode, Options, SampleFrom, Sampling};
use crate::stroke::Stroke;
use crate::tool::Tool;
use crate::wording;

const MIN_GRADIENT: f32 = 1.0;

impl Tab {
    pub(crate) fn draw_refusal(&self) -> Option<Label> {
        if self.session.is_some() {
            return Some(tr!("paint.draw.refusal.session"));
        }
        if !self.takes_selection() {
            return Some(tr!("paint.draw.refusal.busy"));
        }
        self.develop_refusal()
    }

    pub(crate) fn paint_refusal(&self, tool: Tool, options: &Options) -> Option<Label> {
        if !tool.paints() {
            return None;
        }
        self.painting_refusal(
            tool == Tool::Gradient && options.gradient.mode == GradientMode::Transparency,
        )
    }

    fn painting_refusal(&self, fades: bool) -> Option<Label> {
        self.draw_refusal()
            .or_else(|| (fades && !self.holds_transparency()).then(opaque_refusal))
    }

    pub(crate) fn holds_transparency(&self) -> bool {
        let Some(facts) = self.facts() else {
            return false;
        };
        match facts.format {
            FileFormat::Photoshop { .. } => {
                !self.active_layer().is_some_and(|layer| layer.background)
            }
            FileFormat::Picture(_) => !facts.opaque,
        }
    }

    pub(crate) fn stroke(&mut self, stroke: Stroke) -> Option<Label> {
        if let Some(refusal) = self.draw_refusal() {
            return Some(refusal);
        }
        if !stroke.is_worth_keeping() {
            return None;
        }
        if let Some(form) = stroke.form() {
            self.add_figures(self.active_layer?, vec![form], AddMethod::Draw);
            self.ink.settle(stroke, self.ordered);
            return None;
        }
        let (layer, context) = (self.active_layer?, self.context()?);
        let drawn = Recipe::new().then(Marks::new(stroke.mark(Point::ZERO)));
        let recipe = match context.masked(&drawn) {
            Ok(recipe) => recipe,
            Err(error) => return Some(wording::error_message(&error)),
        };
        self.order(Edit::Develop { layer, recipe });
        self.ink.settle(stroke, self.ordered);
        None
    }

    pub(crate) fn gradient(&mut self, gradient: Gradient, finished: bool) -> Option<Label> {
        let refusal = self.painting_refusal(matches!(gradient.ink(), GradientInk::Fade { .. }));
        if refusal.is_some() {
            self.cancel_gradient();
            return refusal;
        }
        let (from, to) = gradient.ends();
        if from.distance(to) < MIN_GRADIENT {
            if finished {
                self.cancel_gradient();
            }
            return None;
        }
        let (layer, context) = (self.active_layer?, self.context()?);
        let recipe = match context.masked(&Recipe::new().then(gradient)) {
            Ok(recipe) => recipe,
            Err(error) => return Some(wording::error_message(&error)),
        };
        match finished {
            true => {
                self.order(Edit::Develop { layer, recipe });
                self.end_preview(Some(self.ordered));
            }
            false => {
                let scale = self.camera.scale();
                if let Some(worker) = &self.worker {
                    self.previews.request_id += 1;
                    self.previews.scale = scale;
                    worker.preview(self.previews.request_id, layer, recipe, scale);
                }
            }
        }
        None
    }

    pub(crate) fn cancel_gradient(&mut self) {
        if self.session.is_none() {
            self.end_preview(None);
        }
    }

    pub(crate) fn fill_selection(&mut self, color: Color) {
        if let Some(layer) = self.active_layer
            && self.edits()
            && self.develop_refusal().is_none()
            && !self.selection.is_empty()
        {
            self.order(Edit::Fill { layer, color });
        }
    }

    pub(crate) fn sample(&mut self, at: (u32, u32), slot: Slot, sampling: Sampling) {
        let Some(worker) = &self.worker else {
            return;
        };
        if !self.is_loaded() {
            return;
        }
        let layer = match sampling.from {
            SampleFrom::Image => None,
            SampleFrom::Layer => self.active_layer,
        };
        self.sampled.request_id += 1;
        self.sampled.slot = slot;
        worker.sample(self.sampled.request_id, at, sampling.size.pixels(), layer);
    }

    pub(crate) fn magic_wand(&mut self, wand: Flooding, at: (f32, f32), combine: Combine) {
        if !self.takes_selection() {
            return;
        }
        if let Some(flood) = wand.flood(at, self.active_layer) {
            self.order_choice(Edit::MagicWand { flood, combine });
        }
    }

    pub(crate) fn pour(&mut self, bucket: Flooding, at: (f32, f32), color: Color) -> Option<Label> {
        if let Some(refusal) = self.draw_refusal() {
            return Some(refusal);
        }
        let layer = self.active_layer?;
        let flood = bucket.flood(at, Some(layer))?;
        self.order(Edit::Bucket {
            layer,
            flood,
            color,
        });
        None
    }
}

fn opaque_refusal() -> Label {
    tr!("paint.draw.refusal.opaque")
}
