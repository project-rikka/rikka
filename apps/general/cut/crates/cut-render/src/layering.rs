use cut_project::{PresetTransform, Shot};
use rikka_figure::{Affine, enclosing};
use rikka_image_edit::{Imaging, Place, Step};
use rikka_types::Rect;

use crate::blanks::Blanks;
use crate::pieces::Pieces;
use crate::plan::Layer;
use crate::request::Request;
use crate::sight::{Cover, Sight};
use crate::wiping::{clip_to_wipes, wipes};

pub(crate) struct Layering<'a> {
    pub(crate) imaging: &'a Imaging,
    pub(crate) pieces: &'a mut Pieces,
    pub(crate) blanks: &'a mut Blanks,
    pub(crate) request: &'a Request,
}

impl Layering<'_> {
    pub(crate) fn sight<'s>(&self, shot: &'s Shot, local: Rect) -> Option<Sight<'s>> {
        Sight::new(
            &shot.projection,
            local,
            self.request.project().format(),
            self.request.factor(),
        )
    }
}

pub(crate) fn placed(
    sight: &Sight<'_>,
    place: Place,
    to_local: Affine,
    transform: &PresetTransform,
    text: bool,
) -> Option<Layer> {
    let extent = extent(&place, to_local);
    let wipes = wipes(transform, text);
    let visible = (!wipes.is_empty())
        .then(|| clip_to_wipes(&wipes, extent.corners().to_vec(), sight.local()));
    let step = Step::new(place);
    match sight.cover(visible, extent) {
        Cover::Whole => Some(Layer::Place(step)),
        Cover::Within(mask) => Some(Layer::Place(step.masked(mask))),
        Cover::Nowhere => None,
    }
}

fn extent(place: &Place, to_local: Affine) -> Rect {
    let (width, height) = place.picture().dimensions();
    let piece = Rect::new(0.0, 0.0, width as f32, height as f32);
    enclosing(piece.corners().map(|corner| to_local.apply(corner)))
}
