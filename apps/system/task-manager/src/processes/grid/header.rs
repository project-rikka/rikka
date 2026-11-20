use std::rc::Rc;

use rikka_app::prelude::*;

use crate::processes::Processes;
use crate::processes::grid::geometry::{
    self, HEADER_HEIGHT, ICON_SIZE, MIN_COLUMN_WIDTH, RESIZE_GRIP_WIDTH,
};
use crate::processes::grid::{Grid, RULE_WIDTH, measure, padded, paint_cell};

pub(crate) struct Header {
    pub(crate) grid: Rc<Grid>,
}

#[derive(Debug, Default)]
pub(crate) struct HeaderState {
    resize: Option<Resize>,
}

#[derive(Clone, Copy, Debug)]
struct Resize {
    column: usize,
    origin: f32,
    width: f32,
}

impl Widget for Header {
    type State = HeaderState;

    fn paint(&self, _state: &mut HeaderState, ctx: &mut PaintCtx<'_>) {
        let rect = ctx.bounds();
        let palette = palette();
        let size = env().font_size;
        let columns = geometry::columns(&self.grid.sizings, rect.size.width);

        ctx.draw_quad(Quad::new(rect).fill(palette.surface));
        ctx.draw_quad(
            Quad::new(Rect::new(
                rect.origin.x,
                rect.max_y() - RULE_WIDTH,
                rect.size.width,
                RULE_WIDTH,
            ))
            .fill(palette.outline),
        );
        let metrics = TextRun::new("", size).no_wrap();

        for (position, sizing) in self.grid.sizings.iter().copied().enumerate() {
            let Some(cell) = geometry::cell(&columns, rect, position, rect.origin.y, HEADER_HEIGHT)
            else {
                continue;
            };
            let sorted = sizing.column == self.grid.sorting.column;
            let Some(title) = self.grid.titles.get(position) else {
                continue;
            };
            let arrow = if sorted { ICON_SIZE } else { 0.0 };
            let text_rect = padded(Rect::new(
                cell.origin.x,
                cell.origin.y,
                (cell.size.width - arrow).max(0.0),
                HEADER_HEIGHT,
            ));
            let color = if sorted {
                palette.text
            } else {
                palette.text_muted
            };
            ctx.clipped(RoundedRect::new(text_rect, 0.0), |ctx| {
                paint_cell(
                    ctx,
                    &metrics,
                    text_rect,
                    title,
                    sizing.column.align(),
                    color,
                );
            });
            if sorted {
                let symbol = if self.grid.sorting.descending {
                    MaterialSymbol::ARROW_DROP_DOWN
                } else {
                    MaterialSymbol::ARROW_DROP_UP
                };
                let at = match sizing.column.align() {
                    TextAlign::Left => text_rect.origin.x + measure(&metrics, title).width,
                    _ => text_rect.max_x() - measure(&metrics, title).width - arrow,
                };
                ctx.draw_icon(
                    Rect::new(at, cell.origin.y, arrow, HEADER_HEIGHT),
                    IconGlyph::from(symbol).spec(ICON_SIZE),
                    palette.accent,
                );
            }
            if position + 1 < columns.len() {
                ctx.draw_quad(
                    Quad::new(Rect::new(
                        cell.max_x() - RULE_WIDTH,
                        cell.origin.y + 8.0,
                        RULE_WIDTH,
                        HEADER_HEIGHT - 16.0,
                    ))
                    .fill(palette.outline),
                );
            }
        }
    }

    fn event(&self, state: &mut HeaderState, event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                ctx.request_focus();
                self.press(state, ctx, position);
            }
            Event::PointerMoved(position) => {
                let Some(resize) = state.resize else {
                    return;
                };
                let width = (resize.width + position.x - resize.origin).max(MIN_COLUMN_WIDTH);
                let column = resize.column;
                ctx.run(move |processes: &mut Processes| processes.resize(column, width));
                ctx.set_handled();
            }
            Event::PointerReleased { .. } if state.resize.take().is_some() => {
                ctx.release_pointer();
            }
            _ => {}
        }
    }
}

impl Header {
    fn press(&self, state: &mut HeaderState, ctx: &mut EventCtx<'_>, at: Point) {
        let bounds = ctx.bounds();
        let columns = geometry::columns(&self.grid.sizings, bounds.size.width);
        let x = at.x - bounds.origin.x;

        for (position, column) in columns.iter().enumerate() {
            if position + 1 < columns.len() && (x - column.max_x()).abs() <= RESIZE_GRIP_WIDTH {
                state.resize = Some(Resize {
                    column: position,
                    origin: at.x,
                    width: self.grid.sizings[position].width,
                });
                ctx.capture_pointer();
                ctx.set_handled();
                return;
            }
        }
        let Some(position) = columns
            .iter()
            .position(|column| x >= column.origin.x && x < column.max_x())
        else {
            return;
        };
        let column = self.grid.sizings[position].column;
        ctx.run(move |processes: &mut Processes| processes.sort(column));
        ctx.set_handled();
    }
}
