use std::rc::Rc;

use rikka_app::prelude::*;

use crate::heat;
use crate::processes::Processes;
use crate::processes::column::Column;
use crate::processes::grid::geometry::{
    self, CELL_PADDING, CHEVRON_SIZE, ICON_SIZE, INDENT_WIDTH, ROW_HEIGHT,
};
use crate::processes::grid::{Grid, HOVERED, RULE_WIDTH, padded, paint_cell};
use crate::processes::row::{Row, RowKind};

pub(crate) struct Line {
    pub(crate) grid: Rc<Grid>,
    pub(crate) index: usize,
}

impl Line {
    fn paint_name(
        &self,
        ctx: &mut PaintCtx<'_>,
        palette: &Palette,
        text: &TextRun,
        row: &Row,
        cell: Rect,
        selected: bool,
    ) {
        let mut x = cell.origin.x + CELL_PADDING + row.indent() * INDENT_WIDTH;
        if self.grid.tree {
            if let RowKind::Leader {
                expandable: true,
                expanded,
            } = row.kind
            {
                let mark = Rect::from_center(
                    Rect::new(x, cell.origin.y, CHEVRON_SIZE, ROW_HEIGHT).center(),
                    Size::new(ICON_SIZE, ICON_SIZE),
                );
                paint_twistie(ctx, mark, expanded, palette.text_muted);
            }
            x += CHEVRON_SIZE;
        }
        if let Some(icon) = row.icon {
            ctx.draw_icon(
                Rect::new(x, cell.origin.y, ICON_SIZE + CELL_PADDING, ROW_HEIGHT),
                IconGlyph::from(icon).spec(ICON_SIZE),
                palette.accent,
            );
            x += ICON_SIZE + CELL_PADDING;
        }
        let label = Rect::new(
            x,
            cell.origin.y,
            (cell.max_x() - CELL_PADDING - x).max(0.0),
            ROW_HEIGHT,
        );
        let color = match row.kind == RowKind::Member {
            true => palette.text_muted,
            false => palette.ink(selected),
        };
        ctx.clipped(RoundedRect::new(label, 0.0), |ctx| {
            paint_cell(ctx, text, label, &row.label, TextAlign::Left, color);
        });
    }

    fn paint_section(&self, ctx: &mut PaintCtx<'_>, palette: &Palette, row: &Row, rect: Rect) {
        ctx.draw_quad(
            Quad::new(Rect::new(
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                RULE_WIDTH,
            ))
            .fill(palette.outline),
        );
        let metrics = TextRun::new("", env().font_size)
            .weight(font::SEMIBOLD)
            .no_wrap();
        let label = Rect::new(
            rect.origin.x + CELL_PADDING,
            rect.origin.y,
            rect.size.width,
            ROW_HEIGHT,
        );
        paint_cell(
            ctx,
            &metrics,
            label,
            &row.label,
            TextAlign::Left,
            palette.text_muted,
        );
    }
}

impl Widget for Line {
    type State = ();

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        let snapshot = self.grid.snapshot_at(self.grid.table.get(), self.index);
        let Some(row) = snapshot.rows.get(self.index) else {
            return;
        };
        let palette = palette();
        let rect = ctx.bounds();
        if let RowKind::Section(_) = row.kind {
            return self.paint_section(ctx, &palette, row, rect);
        }

        let selected = self.grid.selected == Some(row.pid);
        if selected {
            ctx.draw_quad(Quad::new(rect).fill(palette.selection));
        } else if ctx.hovered() {
            ctx.draw_quad(Quad::new(rect).fill(palette.overlay.with_alpha(HOVERED)));
        }

        let text = TextRun::new("", env().font_size).no_wrap();
        let columns = geometry::columns(&self.grid.sizings, rect.size.width);
        for position in 0..columns.len() {
            let column = self.grid.sizings[position].column;
            let (Some(cell), Some(content)) = (
                geometry::cell(&columns, rect, position, rect.origin.y, ROW_HEIGHT),
                row.cell(&snapshot.reading.groups, column, snapshot.scale),
            ) else {
                continue;
            };
            if let Some(shade) = heat::shade(&palette, content.heat) {
                ctx.draw_quad(Quad::new(cell.inflate(-RULE_WIDTH)).fill(shade));
            }
            if column == Column::Name {
                self.paint_name(ctx, &palette, &text, row, cell, selected);
                continue;
            }
            let color = match row.kind == RowKind::Member {
                true => palette.text_muted,
                false => palette.ink(selected),
            };
            paint_cell(
                ctx,
                &text,
                padded(cell),
                &content.text,
                column.align(),
                color,
            );
        }
    }

    fn event(&self, (): &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        let Event::PointerPressed {
            position, button, ..
        } = *event
        else {
            return;
        };
        ctx.request_focus();
        let snapshot = self.grid.snapshot_at(self.grid.table.peek(), self.index);
        let Some(row) = snapshot.rows.get(self.index) else {
            return;
        };
        if !row.selectable() {
            return;
        }
        let bounds = ctx.bounds();
        let pid = row.pid;
        match button {
            PointerButton::Secondary => {
                ctx.run(move |processes: &mut Processes| processes.target_menu(pid));
            }
            PointerButton::Primary => {
                if let RowKind::Leader {
                    expandable: true, ..
                } = row.kind
                    && self.grid.tree
                    && geometry::chevron(bounds, row.indent()).contains(position)
                {
                    ctx.run(move |processes: &mut Processes| processes.toggle(pid));
                    ctx.set_handled();
                    return;
                }
                ctx.run(move |processes: &mut Processes| processes.select(pid));
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER
    }
}
