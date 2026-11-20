use rikka_compositor_geometry::{Logical, Point};

use crate::column::Column;
use crate::direction::{Direction, Side};
use crate::strip::Strip;

impl<W: Copy + PartialEq> Strip<W> {
    pub fn focus(&mut self, direction: Direction) -> Option<W> {
        let column = self.columns.get_mut(self.active)?;
        match direction {
            Direction::Left => {
                let index = self.active.checked_sub(1)?;
                self.activate_index(index);
            }
            Direction::Right => {
                let index = self.active + 1;
                if index >= self.columns.len() {
                    return None;
                }
                self.activate_index(index);
            }
            Direction::Up => column.active = column.active.checked_sub(1)?,
            Direction::Down => {
                if column.active + 1 >= column.cells.len() {
                    return None;
                }
                column.active += 1;
            }
        }
        self.active()
    }

    pub fn shift(&mut self, direction: Direction) -> bool {
        let Some(column) = self.columns.get_mut(self.active) else {
            return false;
        };
        let cell = column.active;
        let target = match direction {
            Direction::Up => cell.checked_sub(1),
            Direction::Down => Some(cell + 1).filter(|index| *index < column.cells.len()),
            Direction::Left | Direction::Right => {
                return self.shift_column(direction == Direction::Right);
            }
        };
        let Some(target) = target else {
            return false;
        };
        column.cells.swap(cell, target);
        column.active = target;
        true
    }

    fn shift_column(&mut self, rightwards: bool) -> bool {
        let to = match rightwards {
            true => Some(self.active + 1).filter(|index| *index < self.columns.len()),
            false => self.active.checked_sub(1),
        };
        let Some(to) = to else {
            return false;
        };
        let view = self.view();
        let column = self.columns.remove(self.active);
        self.columns.insert(to, column);
        self.reseat(to, view);
        true
    }

    pub fn consume_or_expel(&mut self, side: Side) -> bool {
        let Some(source) = self.columns.get(self.active) else {
            return false;
        };
        if !source.is_normal() {
            return false;
        }
        let from = self.active;
        match source.cells.len() {
            1 => self.consume(from, side),
            _ => self.expel(from, side),
        }
    }

    fn consume(&mut self, from: usize, side: Side) -> bool {
        let into = match side {
            Side::Left => from.checked_sub(1),
            Side::Right => Some(from + 1).filter(|index| *index < self.columns.len()),
        };
        let Some(into) = into.filter(|into| self.columns[*into].is_normal()) else {
            return false;
        };
        let view = self.view();
        let window = self.columns.remove(from).active_window();
        let into = match side {
            Side::Left => into,
            Side::Right => from,
        };
        self.columns[into].push(window);
        self.reseat(into, view);
        true
    }

    fn expel(&mut self, from: usize, side: Side) -> bool {
        let source = &mut self.columns[from];
        let window = source.take(source.active);
        let width = source.width;
        let index = match side {
            Side::Left => from,
            Side::Right => from + 1,
        };
        self.insert_column(index, Column::holding(window, width));
        self.activate_index(index);
        true
    }

    pub fn swap(&mut self, one: &W, other: &W) -> bool {
        let (Some(first), Some(second)) = (self.locate(one), self.locate(other)) else {
            return false;
        };
        if first == second
            || !self.columns[first.0].is_normal()
            || !self.columns[second.0].is_normal()
        {
            return false;
        }
        self.columns[first.0].cells[first.1].window = *other;
        self.columns[second.0].cells[second.1].window = *one;
        true
    }

    pub fn drop_at(&mut self, window: &W, at: Point<f64, Logical>) -> bool {
        let tiles = self.tiles();
        let onto = tiles
            .iter()
            .find(|(tile, rect)| tile != window && rect.to_f64().contains(at))
            .map(|(tile, _)| *tile);
        if let Some(onto) = onto {
            return self.swap(window, &onto);
        }
        let Some((from, cell)) = self.locate(window) else {
            return false;
        };
        if !self.columns[from].is_normal() {
            return false;
        }
        let alone = self.columns[from].cells.len() == 1;
        let origin = f64::from(self.frame.work.loc.x);
        let view = self.view();
        let before = self
            .columns
            .iter()
            .zip(self.xs())
            .enumerate()
            .filter(|(index, _)| !(alone && *index == from))
            .filter(|(_, (column, x))| {
                let middle = f64::from(*x) + f64::from(column.extent(&self.frame)) / 2.0;
                origin + middle - view < at.x
            })
            .count();
        let width = self.columns[from].width;
        let window = match alone {
            true => self.columns.remove(from).active_window(),
            false => self.columns[from].take(cell),
        };
        self.columns.insert(before, Column::holding(window, width));
        self.reseat(before, view);
        true
    }
}
