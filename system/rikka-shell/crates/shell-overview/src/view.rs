use rikka_ui::View;

use crate::deck::deck;
use crate::stage::Stage;

pub(crate) fn view(stage: &Stage) -> impl View {
    let board = stage.board();
    let desks = stage.desks(&board);
    deck(board).holding(desks)
}
