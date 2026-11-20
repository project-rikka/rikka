use paint_document::{Edit, Reshape};
use rikka_app::prelude::*;
use rikka_effects_forms::{form_sheet, labelled, note};
use rikka_image_edit::Resampling;

use super::form;
use super::{Context, Dialog, Kind, Outcome, tune};

#[derive(Debug)]
pub(crate) struct Resize {
    from: (u32, u32),
    width: u32,
    height: u32,
    by_percent: bool,
    linked: bool,
    resampling: Resampling,
}

impl Resize {
    pub(crate) fn set_width(&mut self, width: u32) {
        self.width = width.clamp(1, form::MAX_PIXELS as u32);
        if self.linked {
            self.height = scaled(self.from.1, self.width, self.from.0);
        }
    }

    fn set_height(&mut self, height: u32) {
        self.height = height.clamp(1, form::MAX_PIXELS as u32);
        if self.linked {
            self.width = scaled(self.from.0, self.height, self.from.1);
        }
    }

    fn percent(&self) -> (f32, f32) {
        (
            self.width as f32 / self.from.0 as f32 * 100.0,
            self.height as f32 / self.from.1 as f32 * 100.0,
        )
    }

    fn link(&mut self, linked: bool) {
        self.linked = linked;
        if linked {
            self.set_width(self.width);
        }
    }
}

fn scaled(side: u32, to: u32, from: u32) -> u32 {
    (f64::from(side) * f64::from(to) / f64::from(from.max(1)))
        .round()
        .max(1.0) as u32
}

fn scaled_side(side: u32, percent: f32) -> u32 {
    (f64::from(side) * f64::from(percent) / 100.0)
        .round()
        .max(1.0) as u32
}

impl Resize {
    pub(crate) fn from_context(context: &Context<'_>) -> Resize {
        let from = (context.facts.width, context.facts.height);
        Resize {
            from,
            width: from.0,
            height: from.1,
            by_percent: false,
            linked: true,
            resampling: Resampling::default(),
        }
    }
}

impl Dialog for Resize {
    fn title(&self) -> Label {
        Kind::Resize.name()
    }

    fn form(&self, _context: &Context<'_>) -> Element {
        let (width, height) = match self.by_percent {
            true => {
                let (width, height) = self.percent();
                (
                    form::percent(
                        width,
                        tune(|resize: &mut Resize, percent: f32| {
                            resize.set_width(scaled_side(resize.from.0, percent));
                        }),
                    )
                    .any(),
                    form::percent(
                        height,
                        tune(|resize: &mut Resize, percent: f32| {
                            resize.set_height(scaled_side(resize.from.1, percent));
                        }),
                    )
                    .any(),
                )
            }
            false => (
                form::pixels(
                    self.width,
                    tune(|resize: &mut Resize, width: f32| resize.set_width(width.round() as u32)),
                )
                .any(),
                form::pixels(
                    self.height,
                    tune(|resize: &mut Resize, height: f32| {
                        resize.set_height(height.round() as u32);
                    }),
                )
                .any(),
            ),
        };
        form_sheet(vec![
            segmented(usize::from(self.by_percent))
                .segments([
                    tr!("paint.resize.by_pixels"),
                    tr!("paint.resize.by_percent"),
                ])
                .on_select(tune(|resize: &mut Resize, at: usize| {
                    resize.by_percent = at == 1;
                }))
                .fill_width()
                .any(),
            labelled(tr!("paint.resize.width"), width),
            labelled(tr!("paint.resize.height"), height),
            checkbox(self.linked)
                .label(text(tr!("paint.resize.linked")))
                .on_toggle(tune(|resize: &mut Resize, linked: bool| {
                    resize.link(linked)
                }))
                .any(),
            labelled(
                tr!("paint.resize.resampling"),
                form::resampling(
                    self.resampling,
                    tune(|resize: &mut Resize, at: usize| {
                        resize.resampling = Resampling::ALL[at];
                    }),
                ),
            ),
            note(tr!(
                "paint.resize.original",
                width = self.from.0,
                height = self.from.1
            )),
            note(tr!(
                "paint.resize.result",
                width = self.width,
                height = self.height
            )),
        ])
    }

    fn outcome(&self, _context: &Context<'_>) -> Option<Outcome> {
        Some(Outcome::Edit(Edit::Reshape(Reshape::Resize {
            width: self.width,
            height: self.height,
            resampling: self.resampling,
        })))
    }
}
