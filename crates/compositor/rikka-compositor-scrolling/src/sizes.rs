use crate::column::{Column, MAX_EXTENT, MIN_EXTENT, PRESETS, Sizing, Width};
use crate::direction::Edge;
use crate::strip::{Hold, Strip};

impl<W: Copy + PartialEq> Strip<W> {
    pub fn cycle_width(&mut self) -> bool {
        let frame = self.frame;
        let Some(column) = self.columns.get_mut(self.active) else {
            return false;
        };
        if !column.is_normal() {
            return false;
        }
        let next = match column.width {
            Width::Preset(index) => (index + 1) % PRESETS.len(),
            Width::Fixed(_) => {
                let current = column.chosen_extent(&frame);
                PRESETS
                    .iter()
                    .position(|share| current + 1 < frame.proportion(*share))
                    .unwrap_or(0)
            }
        };
        column.width = Width::Preset(next);
        true
    }

    pub fn resize(&mut self, window: &W, edge: Edge, by: f64) -> bool {
        let Some((column, cell)) = self.locate(window) else {
            return false;
        };
        if !self.columns[column].is_normal() || matches!(self.hold, Hold::Swiping(_)) {
            return false;
        }
        self.hold = Hold::Resizing;
        match edge {
            Edge::Left => self.widen(column, -by, true),
            Edge::Right => self.widen(column, by, false),
            Edge::Top => cell
                .checked_sub(1)
                .is_some_and(|above| self.divide(column, above, by)),
            Edge::Bottom => self.divide(column, cell, by),
        }
    }

    pub fn release(&mut self) {
        if matches!(self.hold, Hold::Resizing) {
            self.hold = Hold::Free;
        }
    }

    pub fn enlarge(&mut self, window: &W, sizing: Sizing) -> bool {
        let Some((mut column, cell)) = self.locate(window) else {
            return false;
        };
        if self.columns[column].sizing == sizing {
            return false;
        }
        if sizing != Sizing::Normal && self.columns[column].cells.len() > 1 {
            let focused = column == self.active && self.columns[column].active == cell;
            let width = self.columns[column].width;
            let window = self.columns[column].take(cell);
            self.insert_column(column + 1, Column::holding(window, width));
            column += 1;
            if focused {
                self.activate_index(column);
            }
        }
        if column == self.active {
            match (self.columns[column].sizing, sizing) {
                (Sizing::Normal, _) => self.unenlarged = Some(self.offset),
                (_, Sizing::Normal) => {
                    if let Some(back) = self.unenlarged.take() {
                        self.move_view(back - self.offset);
                    }
                }
                _ => {}
            }
        }
        self.columns[column].sizing = sizing;
        true
    }

    fn widen(&mut self, column: usize, by: f64, keeps_right: bool) -> bool {
        let view = self.view();
        let current = self.columns[column].chosen_width(&self.frame);
        let width = (current + by).clamp(f64::from(MIN_EXTENT), MAX_EXTENT);
        if width == current {
            return false;
        }
        let before = self.columns[column].extent(&self.frame);
        self.columns[column].width = Width::Fixed(width);
        let grown = self.columns[column].extent(&self.frame) - before;
        let view = match keeps_right {
            true => view + f64::from(grown),
            false => view,
        };
        self.set_view(view);
        true
    }

    fn divide(&mut self, column: usize, above: usize, by: f64) -> bool {
        let divided = &mut self.columns[column];
        let below = above + 1;
        if below >= divided.cells.len() {
            return false;
        }
        let spans = divided.spans(&self.frame);
        let pair = spans[above] + spans[below];
        let least = f64::from(MIN_EXTENT);
        if pair < 2.0 * least {
            return false;
        }
        let upper = (spans[above] + by).clamp(least, pair - least);
        if upper == spans[above] {
            return false;
        }
        let mean = spans.iter().sum::<f64>() / spans.len() as f64;
        for (cell, span) in divided.cells.iter_mut().zip(&spans) {
            cell.weight = span / mean;
        }
        divided.cells[above].weight = upper / mean;
        divided.cells[below].weight = (pair - upper) / mean;
        true
    }
}
