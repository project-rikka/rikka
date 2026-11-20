pub(crate) struct Band {
    size: usize,
    width: usize,
    cells: Vec<f64>,
}

impl Band {
    pub(crate) fn new(size: usize, width: usize) -> Self {
        Self {
            size,
            width,
            cells: vec![0.0; size * (width + 1)],
        }
    }

    pub(crate) fn add(&mut self, row: usize, column: usize, value: f64) {
        let (row, column) = (row.max(column), row.min(column));
        let apart = row - column;
        debug_assert!(apart <= self.width);
        self.cells[row * (self.width + 1) + apart] += value;
    }

    fn at(&self, row: usize, column: usize) -> f64 {
        self.cells[row * (self.width + 1) + row - column]
    }

    pub(crate) fn solve(mut self, mut solved: Vec<f64>) -> Option<Vec<f64>> {
        let width = self.width;
        for row in 0..self.size {
            let from = row.saturating_sub(width);
            for column in from..=row {
                let dot: f64 = (from..column)
                    .map(|at| self.at(row, at) * self.at(column, at))
                    .sum();
                let value = self.at(row, column) - dot;
                let slot = row * (width + 1) + row - column;
                if row == column {
                    if value <= 0.0 || !value.is_finite() {
                        return None;
                    }
                    self.cells[slot] = value.sqrt();
                } else {
                    self.cells[slot] = value / self.at(column, column);
                }
            }
        }
        for row in 0..self.size {
            let from = row.saturating_sub(width);
            let dot: f64 = (from..row).map(|at| self.at(row, at) * solved[at]).sum();
            solved[row] = (solved[row] - dot) / self.at(row, row);
        }
        for row in (0..self.size).rev() {
            let to = (row + width + 1).min(self.size);
            let dot: f64 = (row + 1..to).map(|at| self.at(at, row) * solved[at]).sum();
            solved[row] = (solved[row] - dot) / self.at(row, row);
        }
        Some(solved)
    }
}
