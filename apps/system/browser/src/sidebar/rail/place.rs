use browser_store::SplitGroupId;
use rikka_app::prelude::*;

use browser_model::{Row, SpaceId};

pub(crate) const CELL: f32 = 36.0;
pub(crate) const GLYPH: f32 = 18.0;
pub(crate) const PITCH: f32 = CELL + Space::HAIR.px();
const KEY: &str = "rikka-browser-rail";
const BESIDE: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum RailCell {
    Row(Row),
    Group(SplitGroupId),
    NewTab,
    Space(SpaceId),
    NewSpace,
}

pub(crate) fn key_of(cell: RailCell) -> (&'static str, RailCell) {
    (KEY, cell)
}

pub(crate) fn beside(cell: RailCell) -> Anchor {
    Anchor::beside(key_of(cell)).offset(BESIDE, 0.0)
}
