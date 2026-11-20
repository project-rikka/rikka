use cut_project::{Graphic, Shot};
use rikka_image_edit::{Place, Result};

use crate::geometry::stepped_up;
use crate::layering::{Layering, placed};
use crate::letters::poses;
use crate::pieces::{Sketch, Slot};
use crate::plan::Layer;
use crate::sight::Sight;
use crate::stack::Stack;

const ZOOM_STEPS: f32 = 8.0;

impl Layering<'_> {
    pub(crate) fn graphic(
        &mut self,
        shot: &Shot,
        slot: Slot,
        graphic: &Graphic,
        stack: &Stack,
        moving: bool,
    ) -> Result<Option<Layer>> {
        let opacity = graphic.placement.opacity;
        if opacity <= 0.0 {
            return Ok(None);
        }
        let art = &graphic.art;
        let poses = poses(shot, art);
        let rest = self.pieces.reach(slot, art).inflate(stack.reach());
        let scale_of = |sight: &Sight<'_>| match moving {
            true => stepped_up(sight.stretch(), ZOOM_STEPS),
            false => sight.stretch(),
        };
        let local = match poses.is_empty() {
            true => rest,
            false => {
                let hint = self
                    .sight(shot, rest)
                    .map_or(self.request.factor(), |sight| scale_of(&sight));
                match self.pieces.letter_reach(slot, art, &poses, hint) {
                    Some(reach) => reach.inflate(stack.reach()),
                    None => return Ok(None),
                }
            }
        };
        let Some(sight) = self.sight(shot, local) else {
            return Ok(None);
        };
        let scale = scale_of(&sight);
        let sketch = Sketch::new(art, poses, scale);
        let Some(piece) = self
            .pieces
            .graphic(self.imaging, self.blanks, slot, sketch, stack)?
        else {
            return Ok(None);
        };
        let place = Place::new(piece.picture, sight.matrix(piece.to_local)).opacity(opacity);
        Ok(placed(
            &sight,
            place,
            piece.to_local,
            &shot.transform,
            graphic.is_text(),
        ))
    }
}
