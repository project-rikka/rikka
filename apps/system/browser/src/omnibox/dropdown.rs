use rikka_app::prelude::*;

use crate::omnibox::rows::rows;
use crate::omnibox::state::Omnibox;
use crate::topbar::FIELD;
use crate::widgets::presence;

const KEY: &str = "rikka-browser-omnibox-dropdown";
const BELOW: f32 = 6.0;

pub(crate) fn dropdown(omnibox: &Omnibox) -> Element {
    presence(panel(omnibox))
}

fn panel(omnibox: &Omnibox) -> Option<Element> {
    let picker = &omnibox.picker;
    let listing = picker
        .listing()
        .filter(|listing| !listing.rows.is_empty())?;
    Some(
        container(rows(listing, picker.store.as_ref()))
            .glass()
            .background(palette().surface.with_alpha(0.94))
            .border(1.0, palette().outline.with_alpha(0.5))
            .radius(Corner::Panel)
            .transition(pop())
            .build()
            .keyed(KEY)
            .anchored(
                Anchor::new(FIELD, AnchorEdge::BelowStart)
                    .match_target_width()
                    .offset(0.0, BELOW),
            ),
    )
}
