use rikka_compositor_geometry::{Logical, Point, Rect, Size};

use crate::frame::Frame;

pub(crate) const PRESETS: [f64; 3] = [1.0 / 3.0, 0.5, 2.0 / 3.0];
pub(crate) const DEFAULT_PRESET: usize = 1;
pub(crate) const MIN_EXTENT: i32 = 32;
pub(crate) const MAX_EXTENT: f64 = 100_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sizing {
    #[default]
    Normal,
    Maximized,
    Fullscreen,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Width {
    Preset(usize),
    Fixed(f64),
}

impl Default for Width {
    fn default() -> Self {
        Self::Preset(DEFAULT_PRESET)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Cell<W> {
    pub(crate) window: W,
    pub(crate) weight: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Column<W> {
    pub(crate) cells: Vec<Cell<W>>,
    pub(crate) active: usize,
    pub(crate) width: Width,
    pub(crate) sizing: Sizing,
}

impl<W: Copy + PartialEq> Column<W> {
    pub(crate) fn holding(window: W, width: Width) -> Self {
        Self {
            cells: vec![Cell {
                window,
                weight: 1.0,
            }],
            active: 0,
            width,
            sizing: Sizing::Normal,
        }
    }

    pub(crate) fn map<V>(&self, mut map: impl FnMut(W) -> V) -> Column<V> {
        Column {
            cells: self
                .cells
                .iter()
                .map(|cell| Cell {
                    window: map(cell.window),
                    weight: cell.weight,
                })
                .collect(),
            active: self.active,
            width: self.width,
            sizing: self.sizing,
        }
    }

    pub(crate) fn position(&self, window: &W) -> Option<usize> {
        self.cells.iter().position(|cell| cell.window == *window)
    }

    pub(crate) fn active_window(&self) -> W {
        self.cells[self.active].window
    }

    pub(crate) fn is_normal(&self) -> bool {
        self.sizing == Sizing::Normal
    }

    pub(crate) fn extent(&self, frame: &Frame) -> i32 {
        match self.sizing {
            Sizing::Fullscreen => frame.full.size.w,
            Sizing::Maximized => frame.work.size.w,
            Sizing::Normal => self.chosen_extent(frame),
        }
    }

    pub(crate) fn pinned(&self, frame: &Frame, x: f64) -> Option<f64> {
        match self.sizing {
            Sizing::Fullscreen => Some(x + frame.full_offset()),
            Sizing::Maximized => Some(x),
            Sizing::Normal => None,
        }
    }

    pub(crate) fn chosen_extent(&self, frame: &Frame) -> i32 {
        match self.width {
            Width::Preset(index) => frame.proportion(PRESETS[index]),
            Width::Fixed(width) => (width.round() as i32).max(MIN_EXTENT),
        }
    }

    pub(crate) fn chosen_width(&self, frame: &Frame) -> f64 {
        match self.width {
            Width::Preset(_) => f64::from(self.chosen_extent(frame)),
            Width::Fixed(width) => width,
        }
    }

    pub(crate) fn push(&mut self, window: W) {
        self.cells.push(Cell {
            window,
            weight: 1.0,
        });
        self.active = self.cells.len() - 1;
    }

    pub(crate) fn take(&mut self, index: usize) -> W {
        let cell = self.cells.remove(index);
        if index < self.active || self.active == self.cells.len() {
            self.active = self.active.saturating_sub(1);
        }
        if let [only] = self.cells.as_mut_slice() {
            only.weight = 1.0;
        }
        cell.window
    }

    pub(crate) fn spans(&self, frame: &Frame) -> Vec<f64> {
        let room = frame.column_room(self.cells.len());
        let total: f64 = self.cells.iter().map(|cell| cell.weight).sum();
        self.cells
            .iter()
            .map(|cell| room * cell.weight / total)
            .collect()
    }

    pub(crate) fn rects(&self, frame: &Frame, left: i32) -> Vec<(W, Rect<i32, Logical>)> {
        let whole = match self.sizing {
            Sizing::Fullscreen => Some(Rect::new(
                Point::new(left, frame.full.loc.y),
                frame.full.size,
            )),
            Sizing::Maximized => Some(Rect::new(
                Point::new(left, frame.work.loc.y),
                frame.work.size,
            )),
            Sizing::Normal => None,
        };
        if let Some(whole) = whole {
            return self.cells.iter().map(|cell| (cell.window, whole)).collect();
        }
        let width = self.extent(frame);
        let inner = f64::from(frame.spacing.inner);
        let top = f64::from(frame.work.loc.y + frame.spacing.outer);
        let mut reached = 0.0;
        self.spans(frame)
            .into_iter()
            .zip(&self.cells)
            .enumerate()
            .map(|(index, (span, cell))| {
                let gaps = inner * index as f64;
                let start = (top + reached + gaps).round() as i32;
                reached += span;
                let end = (top + reached + gaps).round() as i32;
                let rect = Rect::new(
                    Point::new(left, start),
                    Size::new(width, (end - start).max(1)),
                );
                (cell.window, rect)
            })
            .collect()
    }
}
