use calc_engine::{Engine, Key, Operator, Unary};
use rikka_app::prelude::*;
use rikka_widgets::column;

const KEYS: [[Key; 4]; 6] = [
    [Key::Percent, Key::ClearEntry, Key::Clear, Key::Backspace],
    [
        Key::Unary(Unary::Reciprocal),
        Key::Unary(Unary::Square),
        Key::Unary(Unary::SquareRoot),
        Key::Operator(Operator::Divide),
    ],
    [
        Key::Digit(7),
        Key::Digit(8),
        Key::Digit(9),
        Key::Operator(Operator::Multiply),
    ],
    [
        Key::Digit(4),
        Key::Digit(5),
        Key::Digit(6),
        Key::Operator(Operator::Subtract),
    ],
    [
        Key::Digit(1),
        Key::Digit(2),
        Key::Digit(3),
        Key::Operator(Operator::Add),
    ],
    [
        Key::Unary(Unary::Negate),
        Key::Digit(0),
        Key::Point,
        Key::Equals,
    ],
];

const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

const MIN_ROW_HEIGHT: f32 = 42.0;

pub(crate) fn view(engine: &Engine) -> impl View {
    column! {
        for keys in &KEYS {
            row_of(keys.iter().map(|&key| cell(engine, key)))
                .gap(Space::HAIR)
                .grow(1.0)
                .min_height(MIN_ROW_HEIGHT)
        }
    }
    .gap(Space::HAIR)
    .grow(1.0)
}

fn cell(engine: &Engine, key: Key) -> impl View {
    button(text(label(key)).text_size(type_size(key)))
        .variant(variant(key))
        .enabled(engine.accepts(key))
        .focusless()
        .on_press(move |engine: &mut Engine| engine.press(key))
        .grow(1.0)
        .fill_height()
}

const fn label(key: Key) -> &'static str {
    match key {
        Key::Digit(digit) => DIGITS[digit as usize],
        Key::Point => ".",
        Key::Operator(operator) => operator.symbol(),
        Key::Unary(Unary::Reciprocal) => "1/x",
        Key::Unary(Unary::Square) => "x²",
        Key::Unary(Unary::SquareRoot) => "√x",
        Key::Unary(Unary::Negate) => "+/−",
        Key::Percent => "%",
        Key::Backspace => "⌫",
        Key::ClearEntry => "CE",
        Key::Clear => "C",
        Key::Equals => "=",
    }
}

const fn type_size(key: Key) -> f32 {
    match key {
        Key::Digit(_) | Key::Point | Key::Equals => 19.0,
        Key::Backspace => 17.0,
        _ => 15.0,
    }
}

const fn variant(key: Key) -> ButtonVariant {
    match key {
        Key::Equals => ButtonVariant::Primary,
        _ => ButtonVariant::Secondary,
    }
}
