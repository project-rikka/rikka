use crate::style::{Axis, Layout, Track, Tracks};

use super::edges::{Edges, X, Y, clamp, gap, gaps, self_align, sized};
use super::heights::Sizing;
use super::place::{aligned, justified};
use super::{Heights, Layouts, Node};
use crate::runtime::scratch::Scratch;

struct Columns {
    count: usize,
    size: f32,
}

fn columns(track: Track, inner: f32, gap: f32) -> Columns {
    match track {
        Track::Even(lines) => {
            let count = usize::from(lines.max(1));
            Columns {
                count,
                size: ((inner - gaps(gap, count)) / count as f32).max(0.0),
            }
        }
        Track::Fitting(tile) => {
            let count = (((inner + gap) / (tile + gap)).floor().max(1.0)) as usize;
            Columns {
                count,
                size: ((inner - gaps(gap, count)) / count as f32)
                    .min(tile)
                    .max(0.0),
            }
        }
    }
}

fn lines(track: Track) -> usize {
    match track {
        Track::Even(lines) => usize::from(lines.max(1)),
        Track::Fitting(_) => 1,
    }
}

fn maximize(bases: &[f32], limits: &[f32], free: f32) -> Scratch<f32> {
    let mut sizes: Scratch<f32> = bases.iter().copied().collect();
    let mut open: Scratch<usize> = Scratch::new();
    let mut free = free;
    while free > f32::EPSILON {
        open.clear();
        open.extend((0..sizes.len()).filter(|index| sizes[*index] < limits[*index]));
        if open.is_empty() {
            break;
        }
        let share = free / open.len() as f32;
        let mut spent = 0.0;
        for &index in open.iter() {
            let grown = (sizes[index] + share).min(limits[index]);
            spent += grown - sizes[index];
            sizes[index] = grown;
        }
        if spent <= f32::EPSILON {
            break;
        }
        free -= spent;
    }
    sizes
}

fn starts(sizes: &[f32], gap: f32, from: f32) -> Scratch<f32> {
    let mut at = from;
    sizes
        .iter()
        .map(|size| {
            let start = at;
            at += size + gap;
            start
        })
        .collect()
}

impl Layouts {
    fn least_contribution(&self, child: Node, axis: usize, content: f32) -> f32 {
        let style = &self.style[child.index()];
        let edges = self.edges(child);
        let own = super::edges::length(style.size, axis).fixed();
        let least = match (own, super::edges::scrolls(style, axis)) {
            (None, true) => edges.pad[axis],
            _ => self.inherent(child, axis, content),
        };
        least + edges.margin[axis]
    }

    fn most_contribution(&self, child: Node, axis: usize, content: f32) -> f32 {
        self.inherent(child, axis, content) + self.edges(child).margin[axis]
    }

    fn bands(
        &self,
        cells: &[Node],
        across: usize,
        contribute: impl Fn(Node) -> f32,
    ) -> Scratch<f32> {
        cells
            .chunks(across.max(1))
            .map(|band| {
                band.iter()
                    .map(|cell| contribute(*cell))
                    .fold(0.0, f32::max)
            })
            .collect()
    }

    pub(super) fn grid_widths(&self, node: Node, style: &Layout, tracks: Tracks) -> [f32; 2] {
        let cells: Scratch<Node> = self.in_flow(node).collect();
        let gap = gap(style, X);
        match tracks.along {
            Axis::Vertical => match tracks.track {
                Track::Even(lines) => {
                    let count = usize::from(lines.max(1));
                    let widest = cells
                        .iter()
                        .map(|cell| self.most_contribution(*cell, X, self.widths[cell.index()][1]))
                        .fold(0.0, f32::max);
                    [gaps(gap, count), widest * count as f32 + gaps(gap, count)]
                }
                Track::Fitting(tile) => [0.0, tile],
            },
            Axis::Horizontal => {
                let across = lines(tracks.track);
                let bands = |least: bool| -> f32 {
                    let sizes = match tracks.band {
                        Some(band) => Scratch::filled(band, cells.len().div_ceil(across)),
                        None => self.bands(&cells, across, |cell| {
                            let [low, high] = self.widths[cell.index()];
                            match least {
                                true => self.least_contribution(cell, X, low),
                                false => self.most_contribution(cell, X, high),
                            }
                        }),
                    };
                    sizes.iter().sum::<f32>() + gaps(gap, sizes.len())
                };
                [bands(true), bands(false)]
            }
        }
    }

    pub(super) fn grid_children(
        &mut self,
        node: Node,
        style: &Layout,
        tracks: Tracks,
        inner: f32,
    ) -> Scratch<(Node, f32)> {
        let cells: Scratch<Node> = self.in_flow(node).collect();
        let gap = gap(style, X);
        let across = lines(tracks.track);
        let sizes: Scratch<f32> = match tracks.along {
            Axis::Vertical => {
                let columns = columns(tracks.track, inner, gap);
                Scratch::filled(columns.size, cells.len())
            }
            Axis::Horizontal => {
                let columns = match tracks.band {
                    Some(band) => Scratch::filled(band, cells.len().div_ceil(across)),
                    None => {
                        let limits = self.bands(&cells, across, |cell| {
                            self.most_contribution(cell, X, self.widths[cell.index()][1])
                        });
                        if super::edges::scrolls(style, X) {
                            limits
                        } else {
                            let bases = self.bands(&cells, across, |cell| {
                                self.least_contribution(cell, X, self.widths[cell.index()][0])
                            });
                            let free = inner - bases.iter().sum::<f32>() - gaps(gap, bases.len());
                            maximize(&bases, &limits, free)
                        }
                    }
                };
                let sizes = (0..cells.len())
                    .map(|index| columns[index / across])
                    .collect();
                let tracked = &mut self.tracks[node.index()];
                tracked.clear();
                tracked.extend_from_slice(&columns);
                sizes
            }
        };
        cells
            .iter()
            .zip(sizes.iter())
            .map(|(&cell, &size)| (cell, self.cell_width(cell, size)))
            .collect()
    }

    fn cell_width(&self, cell: Node, area: f32) -> f32 {
        let style = self.style[cell.index()];
        let edges = self.edges(cell);
        let room = Some(area);
        let min = sized(style.min.width, room);
        let max = sized(style.max.width, room);
        match sized(style.size.width, room) {
            Some(own) => clamp(own, min, max).max(edges.pad[X]),
            None => clamp(area - edges.margin[X], min, max),
        }
    }

    fn grid_rows(&self, node: Node, tracks: Tracks) -> (Scratch<Node>, usize) {
        let cells: Scratch<Node> = self.in_flow(node).collect();
        let across = match tracks.along {
            Axis::Vertical => {
                let style = &self.style[node.index()];
                let inner = self.size[node.index()][X] - self.edges(node).inset[X];
                columns(tracks.track, inner, gap(style, X)).count
            }
            Axis::Horizontal => lines(tracks.track),
        };
        (cells, across)
    }

    pub(super) fn grid_heights(&self, node: Node, style: &Layout, tracks: Tracks) -> Heights {
        let (cells, across) = self.grid_rows(node, tracks);
        let gap = gap(style, Y);
        match tracks.along {
            Axis::Vertical => {
                let sum = |sizing: Sizing| -> f32 {
                    let rows = match tracks.band {
                        Some(band) => Scratch::filled(band, cells.len().div_ceil(across)),
                        None => self.bands(&cells, across, |cell| {
                            let heights = self.heights[cell.index()];
                            match sizing {
                                Sizing::Least => self.least_contribution(cell, Y, heights.least),
                                Sizing::Most | Sizing::Offered => {
                                    self.most_contribution(cell, Y, heights.most)
                                }
                            }
                        }),
                    };
                    rows.iter().sum::<f32>() + gaps(gap, rows.len())
                };
                Heights {
                    least: sum(Sizing::Least),
                    most: sum(Sizing::Most),
                    offered: sum(Sizing::Offered),
                }
            }
            Axis::Horizontal => {
                let tallest = cells
                    .iter()
                    .map(|cell| self.most_contribution(*cell, Y, self.heights[cell.index()].most))
                    .fold(0.0, f32::max);
                let most = tallest * across as f32 + gaps(gap, across);
                Heights {
                    least: gaps(gap, across),
                    most,
                    offered: most,
                }
            }
        }
    }

    pub(super) fn place_grid(
        &mut self,
        node: Node,
        style: &Layout,
        edges: &Edges,
        tracks: Tracks,
    ) -> Scratch<(Node, [f32; 2])> {
        let [width, height] = self.size[node.index()];
        let inner = [width - edges.inset[X], height - edges.inset[Y]];
        let (cells, across) = self.grid_rows(node, tracks);
        let gap_x = gap(style, X);
        let gap_y = gap(style, Y);
        let (columns, rows): (Scratch<f32>, Scratch<f32>) = match tracks.along {
            Axis::Vertical => {
                let columns = columns(tracks.track, inner[X], gap_x);
                let rows = match tracks.band {
                    Some(band) => Scratch::filled(band, cells.len().div_ceil(across)),
                    None => {
                        let limits = self.bands(&cells, across, |cell| {
                            self.most_contribution(cell, Y, self.heights[cell.index()].most)
                        });
                        if super::edges::scrolls(style, Y) {
                            limits
                        } else {
                            let bases = self.bands(&cells, across, |cell| {
                                self.least_contribution(cell, Y, self.heights[cell.index()].least)
                            });
                            let free =
                                inner[Y] - bases.iter().sum::<f32>() - gaps(gap_y, bases.len());
                            maximize(&bases, &limits, free)
                        }
                    }
                };
                (Scratch::filled(columns.size, columns.count), rows)
            }
            Axis::Horizontal => {
                let size = ((inner[Y] - gaps(gap_y, across)) / across as f32).max(0.0);
                (
                    self.tracks[node.index()].iter().copied().collect(),
                    Scratch::filled(size, across),
                )
            }
        };
        let used_x = columns.iter().sum::<f32>() + gaps(gap_x, columns.len());
        let shift = justified(style.justify, inner[X].max(0.0) - used_x, 0.0, true);
        let lefts = starts(&columns, gap_x, edges.start[X] + shift);
        let tops = starts(&rows, gap_y, edges.start[Y]);
        let mut laid = Scratch::new();
        for (index, &cell) in cells.iter().enumerate() {
            let (column, row) = match tracks.along {
                Axis::Vertical => (index % across, index / across),
                Axis::Horizontal => (index / across, index % across),
            };
            let area = rows.get(row).copied().unwrap_or(0.0);
            let cell_style = self.style[cell.index()];
            let cell_edges = self.edges(cell);
            let room = Some(area);
            let min = sized(cell_style.min.height, room);
            let max = sized(cell_style.max.height, room);
            let align = self_align(&cell_style, style);
            let cell_height = match sized(cell_style.size.height, room) {
                Some(own) => clamp(own, min, max).max(cell_edges.pad[Y]),
                None if align == crate::style::Align::Stretch => {
                    clamp(area - cell_edges.margin[Y], min, max)
                }
                None => clamp(self.heights[cell.index()].offered, min, max).max(cell_edges.pad[Y]),
            };
            let cross = aligned(align, area - cell_height - cell_edges.margin[Y]);
            let cell_width = self.size[cell.index()][X];
            let area = [
                lefts.get(column).copied().unwrap_or(edges.start[X]),
                tops.get(row).copied().unwrap_or(edges.start[Y]),
            ];
            self.area[cell.index()] = area;
            self.at[cell.index()] = [
                area[X] + cell_edges.margin_start[X],
                area[Y] + cross + cell_edges.margin_start[Y],
            ];
            laid.push((cell, [cell_width, cell_height]));
        }
        laid
    }
}
