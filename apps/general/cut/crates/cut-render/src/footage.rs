use cut_project::{ClipId, Shot, Treatment};
use rikka_figure::Affine;
use rikka_image_edit::{Picture, Place, Result};
use rikka_types::Size;

use crate::geometry::{LARGEST, centred};
use crate::layering::{Layering, placed};
use crate::pieces::{Framing, Slot};
use crate::plan::Layer;
use crate::stack::Stack;

const UNSEEN: f32 = 0.5;

impl Layering<'_> {
    pub(crate) fn footage(
        &mut self,
        shot: &Shot,
        id: ClipId,
        frame: &Picture,
        treatment: &Treatment,
        fitted: Size,
        stack: &Stack,
    ) -> Result<Option<Layer>> {
        let opacity = treatment.placement.opacity;
        if opacity <= 0.0 {
            return Ok(None);
        }
        let local = centred(fitted.width, fitted.height).inflate(stack.reach());
        let Some(sight) = self.sight(shot, local) else {
            return Ok(None);
        };
        let drawn = sight.stretch();
        if fitted.width * drawn < UNSEEN || fitted.height * drawn < UNSEEN {
            return Ok(None);
        }
        let (sideways, upright) = treatment.mirror.signs();
        let (place, to_local) = match stack.is_empty() {
            true => {
                let (width, height) = frame.dimensions();
                let across = fitted.width / width as f32;
                let down = fitted.height / height as f32;
                let to_local =
                    Affine::translation(-f64::from(width) / 2.0, -f64::from(height) / 2.0).then(
                        Affine::scale(f64::from(across * sideways), f64::from(down * upright)),
                    );
                let place = Place::new(frame.clone(), sight.matrix(to_local))
                    .radius(treatment.radius / across.min(down));
                (place, to_local)
            }
            false => {
                let most = stack.padding(drawn) as f32 * 2.0;
                let shrink = (LARGEST / ((fitted.width * drawn).ceil() + most))
                    .min(LARGEST / ((fitted.height * drawn).ceil() + most))
                    .min(1.0);
                let drawn = drawn * shrink;
                let framing = Framing {
                    width: (fitted.width * drawn).ceil() as u32,
                    height: (fitted.height * drawn).ceil() as u32,
                    pad: stack.padding(drawn),
                    radius: treatment.radius * drawn,
                    mirror: treatment.mirror,
                };
                let picture = self.pieces.footage(
                    self.blanks,
                    Slot::Clip(id),
                    frame,
                    framing,
                    stack,
                    drawn,
                )?;
                let pad = framing.pad as f64;
                let to_local = Affine::translation(
                    -f64::from(framing.width) / 2.0 - pad,
                    -f64::from(framing.height) / 2.0 - pad,
                )
                .then(Affine::scale(
                    f64::from(fitted.width) / f64::from(framing.width),
                    f64::from(fitted.height) / f64::from(framing.height),
                ));
                (Place::new(picture, sight.matrix(to_local)), to_local)
            }
        };
        Ok(placed(
            &sight,
            place.opacity(opacity),
            to_local,
            &shot.transform,
            false,
        ))
    }
}
