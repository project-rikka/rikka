use std::rc::Rc;

use iced_core::text::Renderer as TextRenderer;
use iced_core::{Alignment, Background, Border, Color, Element, Font, Length, Theme};
use iced_widget::{button, column, container, lazy, row, scrollable, text};
use ui_scene::{
    BACKGROUND, CELL_RADIUS, GRID_GAP, Grid, REMOVE_WIDTH, ROW_BACKGROUND, ROW_HEIGHT, ROW_HOVERED,
    ROW_ID_WIDTH, ROW_SELECTED, ROW_TEXT_SIZE, Row, TEXT, Table, rgb,
};

pub const LIST_ID: &str = "rows";

#[derive(Clone, Copy, Debug)]
pub enum Message {
    Frame,
    Select(usize),
    Remove(usize),
}

pub type View<'a, R> = Element<'a, Message, Theme, R>;

pub fn color(color: u32) -> Color {
    let [red, green, blue] = rgb(color);
    Color::from_rgb8(red, green, blue)
}

pub fn grid<'a, R>(grid: &Grid) -> View<'a, R>
where
    R: TextRenderer<Font = Font> + 'a,
{
    let layout = grid.layout();
    let cells = grid.cells().map(|cell| {
        let background = color(cell.color);
        container(text(cell.label).size(layout.text_size).color(color(TEXT)))
            .center_x(Length::Fixed(layout.cell_width))
            .center_y(Length::Fixed(layout.cell_height))
            .style(move |_| {
                container::Style::default()
                    .background(background)
                    .border(Border::default().rounded(CELL_RADIUS))
            })
            .into()
    });
    container(
        row(cells)
            .spacing(GRID_GAP)
            .wrap()
            .vertical_spacing(GRID_GAP),
    )
    .padding(GRID_GAP)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style::default().background(color(BACKGROUND)))
    .into()
}

pub fn table<'a, R>(table: &Table) -> View<'a, R>
where
    R: TextRenderer<Font = Font> + 'a,
{
    let rows = table.rows.iter().map(|row| {
        let background = match table.is_selected(row) {
            true => ROW_SELECTED,
            false => ROW_BACKGROUND,
        };
        container(row_content(row))
            .width(Length::Fill)
            .height(Length::Fixed(ROW_HEIGHT))
            .style(move |_| container::Style::default().background(color(background)))
            .into()
    });
    container(scrollable(column(rows).width(Length::Fill)).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style::default().background(color(BACKGROUND)))
        .into()
}

pub fn list<'a, R>(rows: Rc<[Row]>) -> View<'a, R>
where
    R: TextRenderer<Font = Font> + 'static,
{
    let count = rows.len();
    let body = lazy(count, move |_| {
        column(rows.iter().map(list_row::<R>)).width(Length::Fill)
    });
    container(
        scrollable(body)
            .id(LIST_ID)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style::default().background(color(BACKGROUND)))
    .into()
}

fn list_row<R>(row: &Row) -> View<'static, R>
where
    R: TextRenderer<Font = Font> + 'static,
{
    button(row_content(row))
        .padding(0)
        .width(Length::Fill)
        .height(Length::Fixed(ROW_HEIGHT))
        .on_press(Message::Select(row.id))
        .style(|_, status| {
            let background = match status {
                button::Status::Hovered | button::Status::Pressed => ROW_HOVERED,
                button::Status::Active | button::Status::Disabled => ROW_BACKGROUND,
            };
            button::Style {
                background: Some(Background::Color(color(background))),
                text_color: color(TEXT),
                ..button::Style::default()
            }
        })
        .into()
}

fn row_content<'a, R>(row: &Row) -> View<'a, R>
where
    R: TextRenderer<Font = Font> + 'a,
{
    row![
        text(row.id.to_string())
            .size(ROW_TEXT_SIZE)
            .color(color(TEXT))
            .width(Length::Fixed(ROW_ID_WIDTH)),
        text(row.label.clone())
            .size(ROW_TEXT_SIZE)
            .color(color(TEXT))
            .width(Length::Fill),
        button(text("×").size(ROW_TEXT_SIZE))
            .width(Length::Fixed(REMOVE_WIDTH))
            .on_press(Message::Remove(row.id)),
    ]
    .align_y(Alignment::Center)
    .height(Length::Fill)
    .into()
}
