mod letterforms;
mod lettering;
mod pose;
mod raster;
mod typeset;
mod vector;

use rikka_types::Rect;

pub use letterforms::Letterforms;
pub use lettering::{Band, Lettering, Shadow, TextAlign};
pub use pose::{LetterPose, LetterWipe};
pub use raster::Raster;
pub use typeset::{Measure, TextHit, Typeset};

const SHADOW_SPREAD: f32 = 3.0;

#[must_use]
pub(crate) fn decorated_bounds(lettering: &Lettering, typeset: &Typeset) -> Rect {
    let measure = typeset.measure();
    let layout = measure.layout;
    let body = measure
        .ink
        .map_or(layout, |ink| ink.union(&layout))
        .inflate(lettering.outline_reach());
    let body = match lettering.band {
        Some(band) => body.union(&layout.inflate(band.padding)),
        None => body,
    };
    match lettering.shadow {
        Some(shadow) => body.union(
            &body
                .translate(shadow.offset)
                .inflate(shadow.blur * SHADOW_SPREAD),
        ),
        None => body,
    }
}
