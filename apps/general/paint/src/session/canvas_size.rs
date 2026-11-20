use paint_document::{Edit, Reshape};
use rikka_app::prelude::*;
use rikka_effects_forms::{form_sheet, labelled, note};
use rikka_image_edit::Anchor as Pin;

use super::{Context, Dialog, Kind, Outcome, form, tap, tune};

const PINS: [Pin; 9] = [
    Pin::TopLeft,
    Pin::Top,
    Pin::TopRight,
    Pin::Left,
    Pin::Center,
    Pin::Right,
    Pin::BottomLeft,
    Pin::Bottom,
    Pin::BottomRight,
];
const CELL: f32 = 30.0;
const GLYPH: f32 = 16.0;
const ACTIVE_TINT: f32 = 0.22;

#[derive(Debug)]
pub(crate) struct CanvasSize {
    from: (u32, u32),
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pin: Pin,
    fill: Color,
}

pub(crate) fn extension_fill(opaque: bool) -> Color {
    match opaque {
        true => Color::WHITE,
        false => Color::TRANSPARENT,
    }
}

impl CanvasSize {
    pub(crate) fn from_context(context: &Context<'_>) -> CanvasSize {
        let from = (context.facts.width, context.facts.height);
        CanvasSize {
            from,
            width: from.0,
            height: from.1,
            pin: Pin::Center,
            fill: extension_fill(context.facts.opaque),
        }
    }
}

impl Dialog for CanvasSize {
    fn title(&self) -> Label {
        Kind::CanvasSize.name()
    }

    fn form(&self, _context: &Context<'_>) -> Element {
        form_sheet(vec![
            labelled(
                tr!("paint.canvas_size.width"),
                form::pixels(
                    self.width,
                    tune(|canvas: &mut CanvasSize, width: f32| {
                        canvas.width = width.round().max(1.0) as u32;
                    }),
                ),
            ),
            labelled(
                tr!("paint.canvas_size.height"),
                form::pixels(
                    self.height,
                    tune(|canvas: &mut CanvasSize, height: f32| {
                        canvas.height = height.round().max(1.0) as u32;
                    }),
                ),
            ),
            labelled(tr!("paint.canvas_size.anchor"), pins(self.pin)),
            labelled(
                tr!("paint.canvas_size.fill"),
                form::swatch(
                    self.fill,
                    tune(|canvas: &mut CanvasSize, fill: Color| canvas.fill = fill),
                ),
            ),
            note(tr!(
                "paint.resize.original",
                width = self.from.0,
                height = self.from.1
            )),
        ])
    }

    fn outcome(&self, _context: &Context<'_>) -> Option<Outcome> {
        Some(Outcome::Edit(Edit::Reshape(Reshape::CanvasSize {
            width: self.width,
            height: self.height,
            anchor: self.pin,
            fill: self.fill,
        })))
    }
}

fn pins(selected: Pin) -> impl View {
    let at = |pin: Pin| {
        let index = PINS
            .iter()
            .position(|candidate| *candidate == pin)
            .unwrap_or(4) as i32;
        (index % 3, index / 3)
    };
    let (selected_x, selected_y) = at(selected);
    let cells = PINS.map(|pin| {
        let (x, y) = at(pin);
        let symbol = arrow(x - selected_x, y - selected_y);
        let fill = match pin == selected {
            true => palette().accent.scale_alpha(ACTIVE_TINT),
            false => Color::TRANSPARENT,
        };
        let glyph = symbol.map(|symbol| {
            icon(symbol)
                .extent(Length::px(GLYPH))
                .color(palette().text_muted)
        });
        container(glyph)
            .center()
            .background(fill)
            .radius(Corner::Detail)
            .border(1.0, palette().outline)
            .on_press(tap(move |canvas: &mut CanvasSize| canvas.pin = pin))
            .focusless()
            .size(CELL, CELL)
    });
    grid_of(3, cells).gap(Space::HAIR).width(CELL * 3.0 + 8.0)
}

fn arrow(x: i32, y: i32) -> Option<MaterialSymbol> {
    match (x, y) {
        (0, 0) => Some(MaterialSymbol::IMAGE),
        (0, -1) => Some(MaterialSymbol::NORTH),
        (0, 1) => Some(MaterialSymbol::SOUTH),
        (-1, 0) => Some(MaterialSymbol::WEST),
        (1, 0) => Some(MaterialSymbol::EAST),
        (-1, -1) => Some(MaterialSymbol::NORTH_WEST),
        (1, -1) => Some(MaterialSymbol::NORTH_EAST),
        (-1, 1) => Some(MaterialSymbol::SOUTH_WEST),
        (1, 1) => Some(MaterialSymbol::SOUTH_EAST),
        _ => None,
    }
}
