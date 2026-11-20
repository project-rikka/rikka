use calc_engine::Engine;
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::keyboard::keyboard_input;
use crate::{display, keypad};

#[derive(Debug, Default, State)]
pub(crate) struct Calculator {
    engine: Engine,
}

pub(crate) fn view(calculator: &Calculator) -> impl View {
    keyboard_input(
        column! {
            display::view(&calculator.engine);
            keypad::view(&calculator.engine);
        }
        .gap(Space::SNUG)
        .padding(Space::NORMAL)
        .fill(),
    )
}
