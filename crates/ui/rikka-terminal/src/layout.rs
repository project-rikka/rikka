use alacritty_terminal::event::WindowSize;
use alacritty_terminal::grid::Dimensions;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Layout {
    columns: usize,
    lines: usize,
    cell_width: f32,
    cell_height: f32,
}

impl Layout {
    const FLOOR: usize = 1;

    pub(crate) const DEFAULT: Layout = Layout {
        columns: 80,
        lines: 24,
        cell_width: 8.0,
        cell_height: 17.0,
    };

    pub(crate) fn new(columns: usize, lines: usize, cell_width: f32, cell_height: f32) -> Self {
        Self {
            columns: columns.max(Self::FLOOR),
            lines: lines.max(Self::FLOOR),
            cell_width,
            cell_height,
        }
    }

    pub(crate) fn fit(width: f32, height: f32, cell_width: f32, cell_height: f32) -> Self {
        let columns = (width / cell_width).floor().max(0.0) as usize;
        let lines = (height / cell_height).floor().max(0.0) as usize;
        Self::new(columns, lines, cell_width, cell_height)
    }

    pub(crate) fn window_size(&self) -> WindowSize {
        WindowSize {
            num_cols: self.columns as u16,
            num_lines: self.lines as u16,
            cell_width: self.cell_width.round().max(1.0) as u16,
            cell_height: self.cell_height.round().max(1.0) as u16,
        }
    }
}

impl Dimensions for Layout {
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }

    fn screen_lines(&self) -> usize {
        self.lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}
