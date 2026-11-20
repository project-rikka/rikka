mod tile;

use std::rc::Rc;

use rikka_ui::{Color, Place, Styled, TextStyled, View, text};
use rikka_widgets::{button, column_of, lazy_column, row, row_of, scrollable};
use ui_scene::{
    BACKGROUND, CELL_RADIUS, GRID_GAP, Grid, REMOVE_WIDTH, ROW_BACKGROUND, ROW_HEIGHT, ROW_HOVERED,
    ROW_ID_WIDTH, ROW_SELECTED, ROW_TEXT_SIZE, Row, TEXT, Table,
};

pub use tile::{Tile, tile};

pub fn grid(grid: &Grid) -> impl View + use<> {
    let layout = grid.layout();
    let cells = grid.cells().map(|cell| {
        tile(
            text(cell.label)
                .text_size(layout.text_size)
                .color(Color::hex(TEXT)),
            Color::hex(cell.color),
        )
        .radius(CELL_RADIUS)
        .centered()
        .size(layout.cell_width, layout.cell_height)
    });
    tile(
        row_of(cells)
            .wrap_items()
            .gap(GRID_GAP)
            .padding(GRID_GAP)
            .fill(),
        Color::hex(BACKGROUND),
    )
    .fill()
}

pub fn table(table: &Table) -> impl View + use<> {
    let rows = table.rows.iter().map(|row| {
        let background = match table.is_selected(row) {
            true => ROW_SELECTED,
            false => ROW_BACKGROUND,
        };
        tile(row_content(row), Color::hex(background))
            .height(ROW_HEIGHT)
            .fill_width()
    });
    tile(
        scrollable(column_of(rows).fill_width()).fill(),
        Color::hex(BACKGROUND),
    )
    .fill()
}

pub fn list(rows: Rc<[Row]>, place: &Place) -> impl View + use<> {
    let list = lazy_column(rows.len(), move |slot| {
        tile(row_content(&rows[slot.index]), Color::hex(ROW_BACKGROUND))
            .hovered(Color::hex(ROW_HOVERED))
            .height(ROW_HEIGHT)
            .fill_width()
    })
    .uniform(ROW_HEIGHT)
    .place(place)
    .fill();
    tile(list, Color::hex(BACKGROUND)).fill()
}

fn row_content(row: &Row) -> impl View {
    row! {
        text(row.id.to_string())
            .text_size(ROW_TEXT_SIZE)
            .color(Color::hex(TEXT))
            .width(ROW_ID_WIDTH);
        text(row.label.as_str())
            .text_size(ROW_TEXT_SIZE)
            .color(Color::hex(TEXT))
            .grow(1.0);
        button(text("×").text_size(ROW_TEXT_SIZE))
            .on_press(|| {})
            .width(REMOVE_WIDTH)
    }
    .fill_width()
}
