use std::sync::Arc;

use rikka_ui::{Styled, View, is_exiting};
use rikka_widgets::container;

use crate::board::Board;
use crate::panel::panel;

pub(crate) fn view(board: &Board) -> impl View {
    let shown = board.shown().map(|shown| {
        panel(
            Arc::clone(&shown.laid),
            Arc::clone(&shown.faces),
            shown.switch.selected,
        )
        .on_point(Board::pointed)
        .on_choose(Board::chose)
        .on_dismiss(Board::dismissed)
        .leaving(is_exiting())
    });
    container(shown).fill()
}
