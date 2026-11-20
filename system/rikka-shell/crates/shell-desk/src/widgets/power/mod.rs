mod charges;
mod labels;

pub(crate) use charges::Charging;
use desk_layout::{Power, Size};
use rikka_battery::{Charge, Reading};
use rikka_types::Color;
use rikka_ui::{
    Align, Element, Label, Length, MaterialSymbol, Space, Styled, TextAlign, TextStyled, View,
    palette, text, tr,
};
use rikka_widgets::{column, column_of, icon, row, spacer};
use shell_feeds::{percent, symbols};

use crate::widgets::card::padded;
use crate::widgets::kit::{Header, LINE, display, figures, header};
use crate::widgets::paint::{segments, span};
use crate::widgets::power::charges::{Charges, PowerDevice};
use crate::widgets::power::labels::{condition, named};
use crate::widgets::quiet::quiet;
use crate::widgets::sources::Sources;

const SMALL_PERCENT: f32 = 44.0;
const SMALL_CELLS: usize = 10;
const MEDIUM_PERCENT: f32 = 52.0;
const MEDIUM_CELLS: usize = 20;
const CELL_HEIGHT: f32 = 12.0;
const BOLT: f32 = 14.0;
const SYMBOL: f32 = 16.0;
const LEVEL: f32 = 64.0;
const LEVEL_HEIGHT: f32 = 5.0;
const PERCENT: f32 = 40.0;
const ROWS: usize = 4;
const LOW: f32 = 0.2;

struct Cell {
    symbol: MaterialSymbol,
    name: Label,
    charge: f32,
    charging: bool,
}

#[must_use]
pub(crate) fn view(power: &Power, size: Size, sources: &Sources) -> Element {
    let charges = sources.charging.charges();
    let gadgets = match power.devices {
        true => charges.gadgets.as_slice(),
        false => &[],
    };
    let mut cells: Vec<Cell> = charges.main.iter().map(machine).collect();
    cells.extend(gadgets.iter().map(gadget));
    if cells.is_empty() {
        return quiet(
            MaterialSymbol::POWER,
            tr!("shell.desk.widget.power.plugged"),
        )
        .sized(size);
    }
    match (size, charges.main) {
        (Size::Small, _) => small(&cells[0], &charges),
        (_, Some(main)) if cells.len() == 1 => alone(&cells[0], &main, &charges),
        _ => listed(&cells),
    }
}

fn machine(reading: &Reading) -> Cell {
    Cell {
        symbol: symbols::battery(reading),
        name: tr!("shell.desk.widget.power.this-computer"),
        charge: reading.charge,
        charging: reading.state == Charge::Charging,
    }
}

fn gadget(gadget: &PowerDevice) -> Cell {
    Cell {
        symbol: gadget.symbol,
        name: named(&gadget.name),
        charge: gadget.charge,
        charging: gadget.charging,
    }
}

fn tint(cell: &Cell) -> Color {
    let palette = palette();
    match (cell.charging, cell.charge <= LOW) {
        (true, _) => palette.success,
        (false, true) => palette.danger,
        (false, false) => palette.accent,
    }
}

fn media_header(cell: &Cell) -> Header {
    let symbol = match cell.charging {
        true => MaterialSymbol::BOLT,
        false => cell.symbol,
    };
    header(Some(symbol), cell.name.clone()).tint(tint(cell))
}

fn status_label(cell: &Cell, charges: &Charges) -> Option<Label> {
    match &charges.main {
        Some(main) => Some(condition(main, charges.on_battery)),
        None => cell
            .charging
            .then(|| tr!("shell.desk.widget.power.charging")),
    }
}

fn small(cell: &Cell, charges: &Charges) -> Element {
    padded(
        column! {
            media_header(cell);
            spacer().grow(1.0);
            display(percent(cell.charge), SMALL_PERCENT);
            status_label(cell, charges).map(|shown_text| text(shown_text).caption().muted().no_wrap());
            spacer().grow(1.0);
            segments(cell.charge, SMALL_CELLS, tint(cell))
                .height(Length::px(CELL_HEIGHT))
                .fill_width();
        }
        .gap(LINE)
        .fill(),
    )
}

fn alone(cell: &Cell, main: &Reading, charges: &Charges) -> Element {
    let drawing = main.rate.filter(|watts| *watts > 0.0).map(|watts| {
        tr!(
            "shell.desk.widget.power.drawing",
            watts = format!("{watts:.1}").as_str()
        )
    });
    let media_header = match drawing {
        Some(drawing) => media_header(cell).trailing(drawing),
        None => media_header(cell),
    };
    padded(
        column! {
            media_header;
            spacer().grow(1.0);
            row! {
                display(percent(cell.charge), MEDIUM_PERCENT);
                spacer().grow(1.0);
                text(condition(main, charges.on_battery))
                    .label()
                    .muted()
                    .align(TextAlign::End)
                    .no_wrap()
                    .shrink(1.0);
            }
            .gap(Space::SNUG)
            .align_items(Align::End)
            .fill_width();
            spacer().grow(1.0);
            segments(cell.charge, MEDIUM_CELLS, tint(cell))
                .height(Length::px(CELL_HEIGHT))
                .fill_width();
        }
        .gap(LINE)
        .fill(),
    )
}

fn listed(cells: &[Cell]) -> Element {
    let rows = cells.iter().take(ROWS).map(|cell| {
        row! {
            icon(cell.symbol)
                .extent(Length::px(SYMBOL))
                .color(palette().text_muted);
            text(cell.name.clone()).label().no_wrap().shrink(1.0);
            spacer().grow(1.0);
            if cell.charging {
                icon(MaterialSymbol::BOLT)
                    .extent(Length::px(BOLT))
                    .filled()
                    .color(palette().success);
            }
            span(0.0, cell.charge, tint(cell))
                .width(Length::px(LEVEL))
                .height(Length::px(LEVEL_HEIGHT));
            figures(Label::from(percent(cell.charge)))
                .label()
                .align(TextAlign::End)
                .no_wrap()
                .width(Length::px(PERCENT));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .grow(1.0)
        .fill_width()
        .build()
    });
    padded(
        column! {
            header(
                Some(MaterialSymbol::BATTERY_FULL),
                tr!("shell.desk.widget.power.name"),
            );
            column_of(rows).grow(1.0).fill_width();
        }
        .gap(Space::TIGHT)
        .fill(),
    )
}
