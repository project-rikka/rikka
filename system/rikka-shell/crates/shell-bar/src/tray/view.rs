use rikka_tray::State;
use rikka_types::Rect;
use shell_kit::{HighlightButton, highlight_button};

use crate::cluster::RADIUS;
use crate::state::Bar;
use crate::tray::chevron::Chevron;

#[must_use]
pub(crate) fn chevron(tray: &State, unfolded: bool) -> Option<HighlightButton> {
    if tray.is_empty() {
        return None;
    }
    Some(
        highlight_button(
            Chevron::new(unfolded, tray.needs_attention()),
            |bar: &mut Bar, at: Rect| bar.unfold(at),
        )
        .pressed(unfolded)
        .radius(RADIUS),
    )
}
