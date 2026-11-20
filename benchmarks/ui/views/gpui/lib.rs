use std::rc::Rc;

use gpui::{
    Div, InteractiveElement, IntoElement, ParentElement, Stateful, StatefulInteractiveElement,
    Styled, UniformListScrollHandle, div, px, rgb, uniform_list,
};
use ui_scene::{
    BACKGROUND, CELL_RADIUS, GRID_GAP, Grid, REMOVE_WIDTH, ROW_BACKGROUND, ROW_HEIGHT, ROW_HOVERED,
    ROW_ID_WIDTH, ROW_SELECTED, ROW_TEXT_SIZE, Row, TEXT, Table,
};

pub const LIST_ID: &str = "rows";

pub fn grid(grid: &Grid) -> Div {
    let layout = grid.layout();
    div()
        .size_full()
        .bg(rgb(BACKGROUND))
        .flex()
        .flex_wrap()
        .content_start()
        .gap(px(GRID_GAP))
        .p(px(GRID_GAP))
        .children(grid.cells().map(|cell| {
            div()
                .w(px(layout.cell_width))
                .h(px(layout.cell_height))
                .bg(rgb(cell.color))
                .rounded(px(CELL_RADIUS))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(layout.text_size))
                .text_color(rgb(TEXT))
                .child(cell.label)
        }))
}

pub fn table(table: &Table) -> Stateful<Div> {
    div()
        .id("table")
        .size_full()
        .overflow_y_scroll()
        .bg(rgb(BACKGROUND))
        .text_size(px(ROW_TEXT_SIZE))
        .text_color(rgb(TEXT))
        .child(
            div().flex().flex_col().w_full().children(
                table
                    .rows
                    .iter()
                    .map(|row| table_row(row, table.is_selected(row))),
            ),
        )
}

pub fn list(rows: Rc<[Row]>, scroll: &UniformListScrollHandle) -> impl IntoElement {
    uniform_list(LIST_ID, rows.len(), move |range, _, _| {
        range
            .map(|index| list_row(&rows[index]))
            .collect::<Vec<_>>()
    })
    .track_scroll(scroll)
    .size_full()
    .bg(rgb(BACKGROUND))
    .text_size(px(ROW_TEXT_SIZE))
    .text_color(rgb(TEXT))
}

fn table_row(row: &Row, selected: bool) -> Stateful<Div> {
    let background = match selected {
        true => ROW_SELECTED,
        false => ROW_BACKGROUND,
    };
    row_content(row).bg(rgb(background))
}

fn list_row(row: &Row) -> Stateful<Div> {
    row_content(row)
        .bg(rgb(ROW_BACKGROUND))
        .hover(|style| style.bg(rgb(ROW_HOVERED)))
}

fn row_content(row: &Row) -> Stateful<Div> {
    div()
        .id(row.id)
        .h(px(ROW_HEIGHT))
        .w_full()
        .flex()
        .items_center()
        .child(div().w(px(ROW_ID_WIDTH)).child(row.id.to_string()))
        .child(div().flex_1().child(row.label.clone()))
        .child(
            div()
                .id(("remove", row.id))
                .w(px(REMOVE_WIDTH))
                .flex()
                .justify_center()
                .cursor_pointer()
                .on_click(|_, _, _| {})
                .child("×"),
        )
}
