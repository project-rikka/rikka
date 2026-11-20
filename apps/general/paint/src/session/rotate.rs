use paint_document::{Edit, Reshape};
use rikka_app::prelude::*;
use rikka_effects_forms::{form_sheet, labelled, note};
use rikka_image_edit::Resampling;

use super::canvas_size::extension_fill;
use super::form::{self, Scale, Unit};
use super::{Context, Dialog, Kind, Outcome, tune};

const DEGREES: Scale = Scale::new(-180.0..=180.0).decimals(1).unit(Unit::Degrees);

#[derive(Debug)]
pub(crate) struct Rotate {
    pub(crate) degrees: f32,
    resampling: Resampling,
    fill: Color,
}

impl Rotate {
    pub(crate) fn from_context(context: &Context<'_>) -> Rotate {
        Rotate {
            degrees: 0.0,
            resampling: Resampling::Bicubic,
            fill: extension_fill(context.facts.opaque),
        }
    }
}

impl Dialog for Rotate {
    fn title(&self) -> Label {
        Kind::Rotate.name()
    }

    fn form(&self, _context: &Context<'_>) -> Element {
        form_sheet(vec![
            labelled(
                tr!("paint.rotate.angle"),
                form::sliding(
                    self.degrees,
                    &DEGREES,
                    tune(|rotate: &mut Rotate, degrees: f32| rotate.degrees = degrees),
                ),
            ),
            note(tr!("paint.rotate.clockwise")),
            labelled(
                tr!("paint.resize.resampling"),
                form::resampling(
                    self.resampling,
                    tune(|rotate: &mut Rotate, at: usize| rotate.resampling = Resampling::ALL[at]),
                ),
            ),
            labelled(
                tr!("paint.canvas_size.fill"),
                form::swatch(
                    self.fill,
                    tune(|rotate: &mut Rotate, fill: Color| rotate.fill = fill),
                ),
            ),
        ])
    }

    fn outcome(&self, _context: &Context<'_>) -> Option<Outcome> {
        Some(Outcome::Edit(Edit::Reshape(Reshape::Rotate {
            degrees: self.degrees,
            resampling: self.resampling,
            fill: self.fill,
        })))
    }
}
