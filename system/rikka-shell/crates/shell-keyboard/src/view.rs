use rikka_theme::Corner;
use rikka_ui::{
    Curve, DurationToken, Element, Length, Motion, Property, Space, TextStyled, View, fade,
    is_exiting, offset, palette, text,
};
use rikka_virtual_input::Cap;
use rikka_widgets::{column, column_of, container, icon, row_of, spacer};

use crate::keycap::keycap;
use crate::keypad::Keypad;
use crate::latch::{Latch, Latches};
use crate::layout::{self, Face, Press, Slot};

const UNIT: f32 = 46.0;

const GAP: f32 = 6.0;

const FUNCTION: f32 = 34.0;

const PADDING: f32 = 12.0;

const SYMBOL: f32 = 20.0;

const RISE: f32 = 24.0;

const ARRIVING: Curve = Motion::enter(DurationToken::Medium4).curve(Property::Spatial);

pub(crate) const EXIT_DURATION: DurationToken = DurationToken::Short4;

pub(crate) const WIDTH: f32 = layout::UNITS * UNIT + (layout::UNITS - 1.0) * GAP + PADDING * 2.0;

pub(crate) const HEIGHT: f32 = FUNCTION
    + (layout::ROWS.len() - 1) as f32 * UNIT
    + layout::ROWS.len() as f32 * GAP
    + PADDING * 2.0
    - GAP;

pub(crate) fn view(keypad: &Keypad) -> impl View {
    column! {
        if !is_exiting() {
            board(keypad.latches()).transition(
                (fade() + offset(0.0, RISE))
                    .enter(ARRIVING)
                    .exit(Motion::exit(EXIT_DURATION).curve(Property::Spatial)),
            );
        }
    }
}

fn board(latches: &Latches) -> impl View {
    let rows = column_of(
        layout::ROWS
            .iter()
            .enumerate()
            .map(|(number, keys)| rank(number, keys, latches)),
    )
    .gap(Space::TIGHT);

    container(rows)
        .background(palette().background)
        .radius(Corner::Window)
        .glass()
        .padding(Space::NORMAL)
}

fn height(row: usize) -> f32 {
    match row {
        0 => FUNCTION,
        _ => UNIT,
    }
}

fn rank(number: usize, keys: &[Slot], latches: &Latches) -> impl View {
    let height = height(number);
    row_of(keys.iter().enumerate().map(|(place, slot)| {
        keycap(
            face(*slot, latches),
            width(slot.units),
            height,
            move |keypad: &mut Keypad| keypad.pressed(number, place),
            move |keypad: &mut Keypad| keypad.released(number, place),
        )
        .latch(key_latch(*slot, latches))
    }))
    .gap(Space::TIGHT)
}

fn key_latch(slot: Slot, latches: &Latches) -> Latch {
    match slot.press {
        Press::Holds(modifier) => latches.latch(modifier),
        Press::Locks if latches.is_locked() => Latch::Locked,
        _ => Latch::Off,
    }
}

fn width(units: f32) -> f32 {
    units * UNIT + (units - 1.0) * GAP
}

fn face(slot: Slot, latches: &Latches) -> Element {
    match (slot.press, slot.face) {
        (Press::Types(Cap::Levels(plain, shifted)), _) => {
            text(typed(plain, shifted, latches).to_string())
                .heading()
                .medium()
                .build()
        }
        (_, Some(Face::Word(word))) => text(word).caption().medium().build(),
        (_, Some(Face::Symbol(symbol))) => icon(symbol).extent(Length::px(SYMBOL)).build(),
        (_, None) => spacer().build(),
    }
}

fn typed(plain: char, shifted: char, latches: &Latches) -> char {
    let upper = match plain.is_ascii_alphabetic() {
        true => latches.is_upper(),
        false => latches.active_modifiers().shift,
    };
    match upper {
        true => shifted,
        false => plain,
    }
}
