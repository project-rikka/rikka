use rikka_widgets::column;

use crate::offer::first_line;
use crate::prelude::*;
use crate::servers::offered::Listed;

const GIST: usize = 200;

pub(crate) fn offers(offers: &[Listed]) -> impl View {
    column! {
        for offer in offers {
            column! {
                text(offer.name.clone()).caption().medium();
                if let Some(gist) = offer.about.as_deref().and_then(gist) {
                    text(gist).caption().muted();
                }
            }
            .gap(Space::HAIR)
            .fill_width();
        }
    }
    .gap(Space::SNUG)
    .padding(Inset::edges(Space::TIGHT, 0.0, Space::TIGHT, Space::NORMAL))
    .fill_width()
}

fn gist(description: &str) -> Option<String> {
    Some(rikka_format::truncate(first_line(description)?, GIST).into_owned())
}
