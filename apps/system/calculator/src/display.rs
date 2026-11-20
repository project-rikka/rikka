use calc_engine::{Engine, Error};
use rikka_app::prelude::*;
use rikka_widgets::column;

const SIZE: f32 = 42.0;
const MIN_SIZE: f32 = 20.0;
const FIT_CHARACTERS: f32 = 11.0;
const EXPRESSION_HEIGHT: f32 = 18.0;
const ANSWER_HEIGHT: f32 = 60.0;

pub(crate) fn view(engine: &Engine) -> impl View {
    column! {
        expression(engine);
        answer(engine);
    }
    .gap(Space::TIGHT)
    .padding(Inset::xy(10.0, 12.0))
    .shrink(0.0)
}

fn expression(engine: &Engine) -> impl View {
    text(engine.expression())
        .label()
        .muted()
        .align(TextAlign::End)
        .no_wrap()
        .height(EXPRESSION_HEIGHT)
}

fn answer(engine: &Engine) -> impl View {
    container(result_text(engine))
        .height(ANSWER_HEIGHT)
        .align_x(Align::End)
        .align_y(Align::Center)
}

fn result_text(engine: &Engine) -> Text {
    if let Some(error) = engine.error() {
        return text(message(error))
            .title()
            .color(TextColor::Danger)
            .align(TextAlign::End);
    }

    let display = engine.display();
    let size = size_for(display.chars().count());
    text(display)
        .text_size(size)
        .semibold()
        .align(TextAlign::End)
        .no_wrap()
}

fn message(error: Error) -> Label {
    match error {
        Error::DivisionByZero => tr!("calculator.error.division-by-zero"),
        Error::Undefined => tr!("calculator.error.undefined"),
        Error::InvalidInput => tr!("calculator.error.invalid-input"),
        Error::Overflow => tr!("calculator.error.overflow"),
    }
}

fn size_for(characters: usize) -> f32 {
    if characters as f32 <= FIT_CHARACTERS {
        return SIZE;
    }
    (SIZE * FIT_CHARACTERS / characters as f32).max(MIN_SIZE)
}
