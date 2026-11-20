mod emitter;
mod formula;
mod levels;
mod row;
mod scene;
mod timing;

use std::sync::Arc;

use animate_fluid::Emitter;
use rikka_app::prelude::*;

use crate::Desk;

pub(super) fn liquid(desk: Desk<'_>, emitter: &Arc<Emitter>) -> impl View {
    column_of([
        emitter::card(desk, emitter),
        levels::card(desk, emitter),
        formula::card(desk, emitter),
        scene::card(desk),
    ])
    .gap(Space::SNUG)
    .fill_width()
}
