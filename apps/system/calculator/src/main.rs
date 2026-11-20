mod calculator;
mod display;
mod keyboard;
mod keypad;

use rikka_app::prelude::*;

use crate::calculator::Calculator;

rikka_app::mimalloc!();

fn main() -> rikka_app::Result<()> {
    App::new("rikka-calculator")
        .i18n(rikka_i18n::embedded!())
        .windows(|_: LaunchRequest| window())
        .run()
}

fn window() -> Window<Calculator> {
    Window::new(Calculator::default, calculator::view)
        .title(tr!("calculator.title"))
        .size(320.0, 460.0)
        .min_size(280.0, 420.0)
}
