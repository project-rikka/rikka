use rikka_ui::{Element, View};

use crate::island::body::body;
use crate::island::compact::{compact, cover_target};
use crate::island::cover_transition::CoverTransition;
use crate::island::details::{Details, details};
use crate::island::state::Island;

pub(crate) fn island(island: &Island) -> Element {
    let live_activities = island.live_activities();
    let rest = cover_target(island, &live_activities);
    let seated = island.peek().is_none() && !island.is_open();
    let (card, open) = match details(island, &live_activities) {
        Some(Details { card, cover_target }) => (Some(card), cover_target),
        None => (None, None),
    };
    body(
        compact(island, &live_activities, seated),
        card,
        CoverTransition::new(rest, open, seated),
        Island::hover,
    )
    .build()
}
