use rikka_compositor_geometry::{Logical, Rect};

use crate::column::{Column, Width};
use crate::fit::fitted;
use crate::frame::Frame;
use crate::swipe::Swipe;

#[derive(Debug, Clone, Default)]
pub(crate) enum Hold {
    #[default]
    Free,
    Resizing,
    Swiping(Swipe),
}

#[derive(Debug, Clone)]
pub struct Strip<W> {
    pub(crate) frame: Frame,
    pub(crate) columns: Vec<Column<W>>,
    pub(crate) active: usize,
    pub(crate) offset: f64,
    pub(crate) return_to: Option<f64>,
    pub(crate) unenlarged: Option<f64>,
    pub(crate) hold: Hold,
}

impl<W: Copy + PartialEq> Strip<W> {
    #[must_use]
    pub fn new(frame: Frame) -> Self {
        Self {
            frame,
            columns: Vec::new(),
            active: 0,
            offset: 0.0,
            return_to: None,
            unenlarged: None,
            hold: Hold::Free,
        }
    }

    pub fn reframe(&mut self, frame: Frame) {
        self.frame = frame;
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    #[must_use]
    pub fn contains(&self, window: &W) -> bool {
        self.locate(window).is_some()
    }

    pub fn windows(&self) -> impl Iterator<Item = W> + '_ {
        self.columns
            .iter()
            .flat_map(|column| column.cells.iter().map(|cell| cell.window))
    }

    #[must_use]
    pub fn active(&self) -> Option<W> {
        self.columns.get(self.active).map(Column::active_window)
    }

    pub fn insert(&mut self, window: W, next_to: Option<&W>) {
        let index = next_to
            .and_then(|next_to| self.locate(next_to))
            .map(|(column, _)| column + 1)
            .unwrap_or_else(|| match self.columns.is_empty() {
                true => 0,
                false => self.active + 1,
            });
        let stationary = self.offset;
        let right_after = !self.columns.is_empty() && index == self.active + 1;
        self.insert_column(index, Column::holding(window, Width::default()));
        self.activate_index(index);
        self.return_to = right_after.then_some(stationary);
    }

    #[must_use]
    pub fn arrival(&self, frame: Frame, next_to: Option<&W>) -> Rect<i32, Logical> {
        let mut ghost = self.map(Some);
        ghost.reframe(frame);
        ghost.insert(None, next_to.copied().map(Some).as_ref());
        ghost.settle();
        ghost
            .tiles()
            .into_iter()
            .find_map(|(window, rect)| window.is_none().then_some(rect))
            .unwrap_or(frame.work)
    }

    pub fn remove(&mut self, window: &W) -> bool {
        let Some((column, cell)) = self.locate(window) else {
            return false;
        };
        if self.columns[column].cells.len() > 1 {
            self.columns[column].take(cell);
        } else {
            self.remove_column(column);
        }
        true
    }

    pub fn activate(&mut self, window: &W) -> bool {
        let Some((column, cell)) = self.locate(window) else {
            return false;
        };
        let changed = self.active != column || self.columns[column].active != cell;
        self.columns[column].active = cell;
        self.activate_index(column);
        changed
    }

    pub fn settle(&mut self) {
        if !matches!(self.hold, Hold::Free) || self.columns.is_empty() {
            return;
        }
        let column = &self.columns[self.active];
        let x = self.x_of(self.active);
        let target_view = column.pinned(&self.frame, x).unwrap_or_else(|| {
            fitted(
                self.view(),
                self.frame.span(),
                x,
                f64::from(column.extent(&self.frame)),
                self.frame.outer(),
            )
        });
        self.set_view(target_view);
    }

    #[must_use]
    pub fn tiles(&self) -> Vec<(W, Rect<i32, Logical>)> {
        let view = self.view();
        let origin = f64::from(self.frame.work.loc.x);
        self.columns
            .iter()
            .zip(self.xs())
            .flat_map(|(column, x)| {
                let left = (origin + f64::from(x) - view).round() as i32;
                column.rects(&self.frame, left)
            })
            .collect()
    }

    pub(crate) fn map<V: Copy + PartialEq>(&self, mut map: impl FnMut(W) -> V) -> Strip<V> {
        Strip {
            frame: self.frame,
            columns: self
                .columns
                .iter()
                .map(|column| column.map(&mut map))
                .collect(),
            active: self.active,
            offset: self.offset,
            return_to: self.return_to,
            unenlarged: self.unenlarged,
            hold: Hold::Free,
        }
    }

    pub(crate) fn locate(&self, window: &W) -> Option<(usize, usize)> {
        self.columns
            .iter()
            .enumerate()
            .find_map(|(index, column)| Some((index, column.position(window)?)))
    }

    pub(crate) fn xs(&self) -> Vec<i32> {
        let inner = self.frame.spacing.inner;
        let mut x = 0;
        let mut xs = Vec::with_capacity(self.columns.len() + 1);
        for column in &self.columns {
            xs.push(x);
            x += column.extent(&self.frame) + inner;
        }
        xs.push(x);
        xs
    }

    pub(crate) fn x_of(&self, index: usize) -> f64 {
        let inner = self.frame.spacing.inner;
        let x: i32 = self.columns[..index.min(self.columns.len())]
            .iter()
            .map(|column| column.extent(&self.frame) + inner)
            .sum();
        f64::from(x)
    }

    pub(crate) fn view(&self) -> f64 {
        match self.columns.is_empty() {
            true => self.offset,
            false => self.x_of(self.active) + self.offset,
        }
    }

    pub(crate) fn move_view(&mut self, by: f64) {
        self.offset += by;
        if let Hold::Swiping(swipe) = &mut self.hold {
            swipe.base += by;
        }
    }

    pub(crate) fn set_view(&mut self, view: f64) {
        let by = view - self.view();
        self.move_view(by);
    }

    pub(crate) fn activate_index(&mut self, index: usize) {
        if index != self.active {
            let view = self.view();
            self.reseat(index, view);
        }
    }

    pub(crate) fn reseat(&mut self, index: usize, view: f64) {
        self.active = index;
        self.set_view(view);
        self.return_to = None;
        self.unenlarged = None;
    }

    pub(crate) fn insert_column(&mut self, index: usize, column: Column<W>) {
        if self.columns.is_empty() {
            self.columns.push(column);
            self.active = 0;
            self.move_view(-self.offset);
            return;
        }
        self.columns.insert(index, column);
        if index <= self.active {
            self.active += 1;
            self.return_to = None;
        }
    }

    pub(crate) fn remove_column(&mut self, index: usize) -> Column<W> {
        let view = self.view();
        let column = self.columns.remove(index);
        if self.columns.is_empty() {
            self.active = 0;
            self.move_view(-self.offset);
            self.return_to = None;
            self.unenlarged = None;
            return column;
        }
        if index < self.active {
            self.active -= 1;
            self.return_to = None;
            return column;
        }
        if index > self.active {
            return column;
        }
        self.unenlarged = None;
        match self.return_to.take() {
            Some(back) if index > 0 => {
                self.active = index - 1;
                self.move_view(back - self.offset);
            }
            _ if index == self.columns.len() => {
                self.active = index - 1;
                self.set_view(view);
            }
            _ => {}
        }
        column
    }
}
