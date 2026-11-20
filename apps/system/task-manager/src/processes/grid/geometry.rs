use rikka_types::Rect;

use crate::processes::column::Column;

pub(crate) const HEADER_HEIGHT: f32 = 34.0;
pub(crate) const ROW_HEIGHT: f32 = 28.0;
pub(crate) const CELL_PADDING: f32 = 10.0;
pub(crate) const INDENT_WIDTH: f32 = 22.0;
pub(crate) const CHEVRON_SIZE: f32 = 22.0;
pub(crate) const ICON_SIZE: f32 = 16.0;
pub(crate) const RESIZE_GRIP_WIDTH: f32 = 4.0;
pub(crate) const MIN_COLUMN_WIDTH: f32 = 56.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Sizing {
    pub(crate) column: Column,
    pub(crate) width: f32,
}

impl Sizing {
    pub(crate) fn new(column: Column) -> Self {
        Self {
            column,
            width: column.width(),
        }
    }

    fn flexible(self) -> bool {
        matches!(self.column, Column::Name | Column::Command)
    }
}

pub(crate) fn columns(sizings: &[Sizing], width: f32) -> Vec<Rect> {
    let fixed: f32 = sizings
        .iter()
        .filter(|sizing| !sizing.flexible())
        .map(|sizing| sizing.width)
        .sum();
    let flexible: f32 = sizings
        .iter()
        .filter(|sizing| sizing.flexible())
        .map(|sizing| sizing.width)
        .sum();
    let slack = width - fixed - flexible;
    let mut x = 0.0;
    let mut laid = Vec::with_capacity(sizings.len());
    for sizing in sizings {
        let mut column = sizing.width;
        if sizing.flexible() && flexible > 0.0 {
            column = (column + slack * (sizing.width / flexible)).max(MIN_COLUMN_WIDTH * 2.0);
        }
        laid.push(Rect::new(x, 0.0, column, 0.0));
        x += column;
    }
    laid
}

pub(crate) fn cell(
    columns: &[Rect],
    body: Rect,
    position: usize,
    top: f32,
    height: f32,
) -> Option<Rect> {
    let column = columns.get(position)?;
    Some(Rect::new(
        body.origin.x + column.origin.x,
        top,
        column.size.width,
        height,
    ))
}

pub(crate) fn chevron(body: Rect, indent: f32) -> Rect {
    Rect::new(
        body.origin.x + CELL_PADDING + indent * INDENT_WIDTH,
        body.origin.y,
        CHEVRON_SIZE,
        body.size.height,
    )
}
