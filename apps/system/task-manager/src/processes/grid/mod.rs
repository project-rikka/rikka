mod geometry;
mod header;
mod line;

use std::rc::Rc;

use rikka_app::prelude::*;
use rikka_ui::hash_of;
use rikka_widgets::column;

use crate::processes::Processes;
use crate::processes::column::Sorting;
use crate::processes::row::{Row, RowKind};
use crate::processes::table::{RowTable, Snapshot};

pub(crate) use geometry::Sizing;

use geometry::{CELL_PADDING, HEADER_HEIGHT, ROW_HEIGHT};
use header::Header;
use line::Line;

const LEADING: f32 = 1.4;
const HOVERED: f32 = 0.05;
const RULE_WIDTH: f32 = 1.0;

pub(crate) struct Grid {
    pub(crate) sizings: Vec<Sizing>,
    pub(crate) titles: Vec<String>,
    pub(crate) table: RowTable,
    pub(crate) snapshot: Rc<Snapshot>,
    pub(crate) glides: bool,
    pub(crate) order: u64,
    pub(crate) focus: Option<usize>,
    pub(crate) selected: Option<i32>,
    pub(crate) sorting: Sorting,
    pub(crate) tree: bool,
}

impl Grid {
    pub(crate) fn view(self) -> impl View {
        let (count, focus, glides) = (self.snapshot.rows.len(), self.focus, self.glides);
        let grid = Rc::new(self);
        let shared = grid.shared();
        let (drawn, reached, chosen, keyed, versioned, named) = (
            Rc::clone(&grid),
            Rc::clone(&grid),
            Rc::clone(&grid),
            Rc::clone(&grid),
            Rc::clone(&grid),
            Rc::clone(&grid),
        );

        let head = Element::new(Header {
            grid: Rc::clone(&grid),
        })
        .fill_width()
        .height(HEADER_HEIGHT);

        let lines = lazy_column(count, move |slot| {
            Element::new(Line {
                grid: Rc::clone(&drawn),
                index: slot.index,
            })
            .fill_width()
            .height(ROW_HEIGHT)
        })
        .uniform(ROW_HEIGHT)
        .memo(move |index| {
            let pid = versioned.snapshot.rows.get(index)?.pid;
            Some(Version {
                key: hash_of(index),
                revision: hash_of((shared, versioned.selected == Some(pid))),
            })
        })
        .named(move |index| named.snapshot.rows.get(index).map_or(0, Row::name))
        .glides(glides)
        .focus(focus)
        .reachable(move |index| {
            reached
                .table
                .peek()
                .rows
                .get(index)
                .is_some_and(Row::selectable)
        })
        .on_focus(
            move |processes: &mut Processes, index: usize, _: Modifiers| {
                if let Some(row) = chosen.table.peek().rows.get(index) {
                    processes.select(row.pid);
                }
            },
        )
        .on_key(
            move |processes: &mut Processes, key: Key, _: Modifiers, focus: Option<usize>| {
                keyed.pressed(processes, key, focus)
            },
        )
        .grow(1.0)
        .fill_width();

        column! {
            head;
            lines
        }
        .fill()
    }

    fn shared(&self) -> u64 {
        let widths: Vec<(u8, u32)> = self
            .sizings
            .iter()
            .map(|sizing| (sizing.column as u8, sizing.width.to_bits()))
            .collect();
        hash_of((widths, self.tree, self.order))
    }

    fn snapshot_at(&self, live: Rc<Snapshot>, index: usize) -> Rc<Snapshot> {
        let name = |snapshot: &Snapshot| snapshot.rows.get(index).map(Row::name);
        match name(&live) == name(&self.snapshot) {
            true => live,
            false => Rc::clone(&self.snapshot),
        }
    }

    fn pressed(&self, processes: &mut Processes, key: Key, focus: Option<usize>) -> bool {
        match key {
            Key::ArrowRight | Key::ArrowLeft => {
                let snapshot = self.table.peek();
                let Some(row) = focus.and_then(|focus| snapshot.rows.get(focus)) else {
                    return false;
                };
                let RowKind::Leader {
                    expandable: true,
                    expanded,
                } = row.kind
                else {
                    return false;
                };
                if expanded == (key == Key::ArrowRight) {
                    return false;
                }
                processes.toggle(row.pid);
                true
            }
            Key::Delete => {
                processes.end_task(tasks_process::terminate);
                true
            }
            _ => false,
        }
    }
}

fn paint_cell(
    ctx: &mut PaintCtx<'_>,
    metrics: &TextRun,
    cell: Rect,
    text: &str,
    align: TextAlign,
    color: Color,
) {
    if text.is_empty() {
        return;
    }
    if align == TextAlign::Left {
        let height = env().font_size * LEADING;
        let y = cell.origin.y + (cell.size.height - height) / 2.0;
        ctx.draw_text(
            Rect::new(cell.origin.x, y, cell.size.width, height),
            metrics.clone().content(text),
            color,
        );
        return;
    }
    let size = measure(metrics, text);
    let x = match align {
        TextAlign::Center => cell.center().x - size.width / 2.0,
        _ => cell.max_x() - size.width,
    };
    let y = cell.origin.y + (cell.size.height - size.height) / 2.0;
    ctx.draw_text(
        Rect::new(x, y, size.width, size.height),
        metrics.clone().content(text),
        color,
    );
}

fn padded(cell: Rect) -> Rect {
    Rect::new(
        cell.origin.x + CELL_PADDING,
        cell.origin.y,
        (cell.size.width - CELL_PADDING * 2.0).max(0.0),
        cell.size.height,
    )
}

fn measure(metrics: &TextRun, text: &str) -> Size {
    measure_text(&metrics.clone().content(text), None)
}
