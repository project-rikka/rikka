use crate::Reach;
use animate_body::{Expression, Faces, MorphWeights};
use animate_ui::labelled;
use rikka_app::prelude::*;

use crate::cards::strength;
use crate::wording;

pub(super) fn mouth(reach: Reach, faces: &Faces, weights: &MorphWeights) -> impl View {
    column_of(
        Expression::VOWELS
            .into_iter()
            .filter(|vowel| faces.has(*vowel))
            .map(|vowel| {
                labelled(
                    wording::expression(vowel),
                    strength(reach, vowel, faces.strength(weights, vowel)),
                    None,
                )
            }),
    )
    .gap(Space::TIGHT)
    .fill_width()
}
