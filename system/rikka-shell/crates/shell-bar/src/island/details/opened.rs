use rikka_ui::{Element, Space, Styled, View};
use rikka_widgets::column;

use crate::island::cover_transition::CoverTarget;
use crate::island::details::{idle, media, rows};
use crate::island::live::Live;
use crate::island::state::Island;

const ROWS: usize = 3;

pub(crate) fn view(island: &Island, live_activities: &[Live]) -> (Element, Option<CoverTarget>) {
    let (head, cover_target) = match island.player() {
        Some(player) => (
            media::card(player, island).build(),
            Some(media::cover_target(player)),
        ),
        None => (idle::view(island).build(), None),
    };
    let view = column! {
        head;
        for live in live_activities.iter().filter(|live| !matches!(live, Live::Media(_))).take(ROWS) {
            rows::row(live, island);
        }
    }
    .gap(Space::SNUG)
    .fill_width()
    .build();
    (view, cover_target)
}
