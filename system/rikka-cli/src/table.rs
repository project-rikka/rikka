use std::io::{self, Write};

const GAP: &str = "  ";

#[derive(Debug)]
pub(crate) struct Table<const N: usize> {
    indent: &'static str,
    rows: Vec<[String; N]>,
}

impl<const N: usize> Table<N> {
    pub(crate) fn new(header: [&str; N]) -> Table<N> {
        Table {
            indent: "",
            rows: vec![header.map(str::to_owned)],
        }
    }

    pub(crate) fn without_header(indent: &'static str) -> Table<N> {
        Table {
            indent,
            rows: Vec::new(),
        }
    }

    pub(crate) fn row(&mut self, cells: [String; N]) {
        self.rows.push(cells);
    }

    pub(crate) fn write(&self, out: &mut dyn Write) -> io::Result<()> {
        let mut widths = [0; N];
        for row in &self.rows {
            for (width, cell) in widths.iter_mut().zip(row) {
                *width = (*width).max(cell.chars().count());
            }
        }
        for row in &self.rows {
            let mut line = self.indent.to_owned();
            for (column, cell) in row.iter().enumerate() {
                line.push_str(cell);
                if column + 1 < N {
                    let padding = widths[column] - cell.chars().count();
                    line.extend(std::iter::repeat_n(' ', padding));
                    line.push_str(GAP);
                }
            }
            writeln!(out, "{}", line.trim_end())?;
        }
        Ok(())
    }
}
