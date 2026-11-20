use paint_document::{Combine, FloodMode, Shape};
use rikka_app::prelude::*;
use rikka_image_edit::{Gradient, GradientInk, GradientShape};

use super::Paint;
use crate::colors::{Colors, Slot};
use crate::figures::Restyle;
use crate::options::{Filling, Flooding, GradientMode, Options, SampleFrom, SampleSize, Setting};
use crate::stroke::Stroke;
use crate::tool::Tool;

impl Paint {
    pub(crate) fn colors(&self) -> Colors {
        self.colors
    }

    pub(crate) fn options(&self) -> &Options {
        &self.options
    }

    pub(crate) fn last_sample(&self) -> Option<Color> {
        self.sampled
    }

    pub(crate) fn set_color(&mut self, slot: Slot, color: Color) {
        self.colors.set(slot, color);
        self.restyle(Restyle::Colors(self.colors), false);
    }

    pub(crate) fn commit_color(&mut self, slot: Slot, color: Color) {
        self.colors.set(slot, color);
        self.restyle(Restyle::Colors(self.colors), true);
    }

    pub(crate) fn swap_colors(&mut self) {
        self.colors.swap();
        self.restyle(Restyle::Colors(self.colors), true);
    }

    pub(crate) fn reset_colors(&mut self) {
        self.colors.reset();
        self.restyle(Restyle::Colors(self.colors), true);
    }

    pub(crate) fn draws(&self) -> bool {
        self.styled_tool().sized()
    }

    pub(crate) fn set_width(&mut self, width: f32) {
        self.options.set_width(self.styled_tool(), width);
        self.restyle_width(true);
    }

    pub(crate) fn slide_width(&mut self, width: f32) {
        self.options.set_width(self.styled_tool(), width);
        self.restyle_width(false);
    }

    pub(crate) fn step_width(&mut self, grow: bool) {
        self.options.step_width(self.styled_tool(), grow);
        self.restyle_width(true);
    }

    fn restyle_width(&mut self, commit: bool) {
        if let Some(width) = self.options.width(self.styled_tool()) {
            self.restyle(Restyle::Width(width), commit);
        }
    }

    pub(crate) fn set_brushing(&mut self, setting: Setting, value: f32) {
        self.options.set_brushing(self.tool, setting, value);
    }

    pub(crate) fn set_antialiased(&mut self, antialiased: bool) {
        self.options.antialiased = antialiased;
        self.restyle(Restyle::Antialiased(antialiased), true);
    }

    pub(crate) fn set_gradient_shape(&mut self, shape: GradientShape) {
        self.options.gradient.shape = shape;
    }

    pub(crate) fn set_gradient_mode(&mut self, mode: GradientMode) {
        self.options.gradient.mode = mode;
    }

    pub(crate) fn gradient(&mut self, (from, to): (Point, Point), slot: Slot, finished: bool) {
        if self.unsaved_prompt.is_some() {
            return;
        }
        let gradienting = self.options.gradient;
        let ink = match (gradienting.mode, slot) {
            (GradientMode::Color, _) => GradientInk::Colors {
                from: self.colors.get(slot),
                to: self.colors.get(slot.other()),
            },
            (GradientMode::Transparency, Slot::Primary) => GradientInk::Fade { from: 1.0, to: 0.0 },
            (GradientMode::Transparency, Slot::Secondary) => {
                GradientInk::Fade { from: 0.0, to: 1.0 }
            }
        };
        let gradient = Gradient::new(gradienting.shape, from, to, ink);
        let refusal = self
            .active_tab_mut()
            .and_then(|tab| tab.gradient(gradient, finished));
        match refusal {
            Some(refusal) => self.notice = Some(refusal),
            None if !finished => skip_view(),
            None => {}
        }
    }

    pub(crate) fn cancel_gradient(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            tab.cancel_gradient();
        }
    }

    pub(crate) fn set_filling(&mut self, filling: Filling) {
        self.options.filling = filling;
        self.restyle(
            Restyle::Filling {
                filling,
                colors: self.colors,
                width: self.options.width(self.styled_tool()),
            },
            true,
        );
    }

    pub(crate) fn set_selecting(&mut self, combine: Combine) {
        self.options.selecting = combine;
    }

    pub(crate) fn set_sample_size(&mut self, size: SampleSize) {
        self.options.sampling.size = size;
    }

    pub(crate) fn set_sample_from(&mut self, from: SampleFrom) {
        self.options.sampling.from = from;
    }

    pub(crate) fn cycle_shapes(&mut self) {
        self.pick(self.tool.next_shape());
    }

    pub(crate) fn stroke(&mut self, stroke: Stroke) {
        if self.unsaved_prompt.is_some() {
            return;
        }
        let refusal = self.active_tab_mut().and_then(|tab| tab.stroke(stroke));
        if refusal.is_some() {
            self.notice = refusal;
        }
    }

    pub(crate) fn refuse(&mut self, refusal: Label) {
        self.notice = Some(refusal);
    }

    pub(crate) fn sample(&mut self, at: (u32, u32), slot: Slot) {
        let sampling = self.options.sampling;
        if self.unsaved_prompt.is_none()
            && let Some(tab) = self.active_tab_mut()
        {
            tab.sample(at, slot, sampling);
        }
        skip_view();
    }

    pub(crate) fn fill_selection(&mut self) {
        let color = self.colors.primary;
        self.edit(|tab| tab.fill_selection(color));
    }

    pub(super) fn sampled(&mut self, color: Color, slot: Slot) {
        self.colors.set(slot, color);
        self.sampled = Some(color);
    }

    pub(crate) fn set_smoothing(&mut self, smoothing: bool) {
        self.options.smoothing = smoothing;
    }

    pub(crate) fn set_tolerance(&mut self, tolerance: f32) {
        self.flood_options(|flooding| flooding.tolerance = tolerance.clamp(0.0, 1.0));
    }

    pub(crate) fn set_flood_mode(&mut self, mode: FloodMode) {
        self.flood_options(|flooding| flooding.mode = mode);
    }

    pub(crate) fn set_flood_from(&mut self, from: SampleFrom) {
        self.flood_options(|flooding| flooding.from = from);
    }

    pub(crate) fn set_flood_anti_aliased(&mut self, anti_aliased: bool) {
        self.flood_options(|flooding| flooding.anti_aliased = anti_aliased);
    }

    fn flood_options(&mut self, change: impl FnOnce(&mut Flooding)) {
        match self.tool {
            Tool::MagicWand => change(&mut self.options.wand),
            Tool::PaintBucket => change(&mut self.options.bucket),
            _ => skip_view(),
        }
    }

    pub(crate) fn cycle_selections(&mut self) {
        self.pick(self.tool.next_selection());
    }

    pub(crate) fn select_shape(&mut self, shape: Shape, combine: Combine) {
        self.mark(|tab| tab.select_shape(shape, combine));
    }

    pub(crate) fn magic_wand(&mut self, at: (f32, f32), combine: Combine) {
        let wand = self.options.wand;
        self.mark(|tab| tab.magic_wand(wand, at, combine));
    }

    pub(crate) fn move_selection(&mut self, by: (i32, i32)) {
        self.mark(|tab| tab.move_selection(by));
    }

    pub(crate) fn select_layer_pixels(&mut self) {
        self.mark(super::Tab::select_layer_pixels);
    }

    pub(crate) fn pour(&mut self, at: (f32, f32), slot: Slot) {
        if self.unsaved_prompt.is_some() {
            return;
        }
        let (bucket, color) = (self.options.bucket, self.colors.get(slot));
        let refusal = self
            .active_tab_mut()
            .and_then(|tab| tab.pour(bucket, at, color));
        if refusal.is_some() {
            self.notice = refusal;
        }
    }
}
