use paint_assistant::{Cropping, TransformStep};
use paint_document::{Area, Edit, Region, Reshape};
use rikka_types::Color;

use super::{Composer, Refusal, refused, stepped};

const MAX_SIDE: u32 = 300_000;

impl Composer<'_> {
    pub(super) fn transform(&mut self, steps: Vec<TransformStep>) -> Result<(), Refusal> {
        for (at, step) in steps.into_iter().enumerate() {
            let kind = kind(&step);
            let edit = stepped("step", at, kind, self.reshaped(step))?;
            stepped("step", at, kind, self.apply(&edit))?;
        }
        Ok(())
    }

    fn reshaped(&self, step: TransformStep) -> Result<Edit, Refusal> {
        let (width, height) = self.size();
        let canvas = self.canvas();
        let extension = match self.document.is_opaque() {
            true => Color::WHITE,
            false => Color::TRANSPARENT,
        };
        Ok(match step {
            TransformStep::Crop(cropping) => {
                let region = match cropping {
                    Cropping::Rect(rect) => {
                        Region::rectangle(Area::new(rect.left, rect.top, rect.width, rect.height))
                    }
                    Cropping::Selection => self.document.selection().clone(),
                }
                .clipped(canvas);
                if region.is_empty() {
                    return Err(refused(match cropping {
                        Cropping::Selection => "nothing is selected to crop to; give `rect`",
                        Cropping::Rect(_) => "the crop lies outside the canvas",
                    }));
                }
                Edit::Crop(region)
            }
            TransformStep::Resize {
                width: asked_width,
                height: asked_height,
                resampling,
            } => {
                let (width, height) = proportional(asked_width, asked_height, width, height);
                fits(width, height)?;
                Edit::Reshape(Reshape::Resize {
                    width,
                    height,
                    resampling,
                })
            }
            TransformStep::CanvasSize {
                width: asked_width,
                height: asked_height,
                anchor,
                fill,
            } => {
                let (width, height) =
                    (asked_width.unwrap_or(width), asked_height.unwrap_or(height));
                fits(width, height)?;
                Edit::Reshape(Reshape::CanvasSize {
                    width,
                    height,
                    anchor,
                    fill: fill.unwrap_or(extension),
                })
            }
            TransformStep::Rotate {
                degrees,
                resampling,
                fill,
            } => Edit::Reshape(Reshape::Rotate {
                degrees,
                resampling,
                fill: fill.unwrap_or(extension),
            }),
            TransformStep::Turn(turn) => Edit::Reshape(Reshape::Turn(turn)),
            TransformStep::Flip(flip) => Edit::Reshape(Reshape::Flip(flip)),
            TransformStep::Convert { profile, intent } => Edit::Convert { profile, intent },
        })
    }
}

fn kind(step: &TransformStep) -> &'static str {
    match step {
        TransformStep::Crop(_) => "crop",
        TransformStep::Resize { .. } => "resize",
        TransformStep::CanvasSize { .. } => "canvas_size",
        TransformStep::Rotate { .. } => "rotate",
        TransformStep::Turn(_) => "turn",
        TransformStep::Flip(_) => "flip",
        TransformStep::Convert { .. } => "convert",
    }
}

fn proportional(
    width: Option<u32>,
    height: Option<u32>,
    from_width: u32,
    from_height: u32,
) -> (u32, u32) {
    let scaled = |value: u32, along: u32, across: u32| {
        ((f64::from(value) * f64::from(across) / f64::from(along.max(1))).round() as u32).max(1)
    };
    match (width, height) {
        (Some(width), Some(height)) => (width, height),
        (Some(width), None) => (width, scaled(width, from_width, from_height)),
        (None, Some(height)) => (scaled(height, from_height, from_width), height),
        (None, None) => (from_width, from_height),
    }
}

fn fits(width: u32, height: u32) -> Result<(), Refusal> {
    match width <= MAX_SIDE && height <= MAX_SIDE {
        true => Ok(()),
        false => Err(refused(format!(
            "{width}×{height} is larger than {MAX_SIDE} px on a side"
        ))),
    }
}
