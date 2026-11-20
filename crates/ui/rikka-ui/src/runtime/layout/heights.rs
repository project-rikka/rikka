use crate::style::{Flow, Layout};

use super::edges::{Edges, X, Y, clamp, gap, gaps};
use super::flex::{self, Contribution};
use super::{Heights, Layouts, Node};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Sizing {
    Least,
    Most,
    Offered,
}

impl Heights {
    pub(super) fn resolve(self, sizing: Sizing) -> f32 {
        match sizing {
            Sizing::Least => self.least,
            Sizing::Most => self.most,
            Sizing::Offered => self.offered,
        }
    }
}

impl Layouts {
    pub(super) fn measure_heights(&mut self, node: Node) {
        let at = node.index();
        if !self.visits(node) {
            return;
        }
        for index in 0..self.children[at].len() {
            let child = self.children[at][index];
            self.measure_heights(child);
        }
        self.heights[at] = self.content_heights(node);
    }

    fn content_heights(&mut self, node: Node) -> Heights {
        let at = node.index();
        let style = self.style[at];
        let edges = self.edges(node);
        if self.children[at].is_empty() {
            let pad = edges.pad[Y];
            let Some(text) = self.leaf_size(node) else {
                return Heights {
                    least: pad,
                    most: pad,
                    offered: pad,
                };
            };
            let height = text.height + pad;
            let least = self.leaf[at]
                .as_ref()
                .and_then(|leaf| leaf.least_height())
                .map_or(height, |least| least + pad);
            return Heights {
                least,
                most: height,
                offered: height,
            };
        }
        let heights = match style.flow {
            Flow::Column => self.column_heights(node, &style, &edges),
            Flow::Row => self.row_heights(node, &style),
            Flow::Grid(tracks) => self.grid_heights(node, &style, tracks),
        };
        let min = style.min.height.fixed();
        let max = style.max.height.fixed();
        let inset = edges.inset[Y];
        let settle = |height: f32| clamp(height + inset, min, max).max(edges.pad[Y]);
        Heights {
            least: settle(heights.least),
            most: settle(heights.most),
            offered: settle(heights.offered),
        }
    }

    pub(super) fn leaf_size(&mut self, node: Node) -> Option<rikka_types::Size> {
        let at = node.index();
        let pad = self.edges(node).pad[X];
        let query = self.size[at][X] - pad;
        let natural = self.natural[at];
        let fit = self.fits[at].take();
        let leaf = self.leaf[at].as_mut()?;
        let size = if !leaf.wraps() || query >= natural.width {
            natural
        } else {
            match fit {
                Some((max_width, size)) if size.width <= query && query <= max_width => size,
                _ => leaf.size_for_width(query),
            }
        };
        self.fits[at] = Some((query, size));
        Some(size)
    }

    fn column_heights(&self, node: Node, style: &Layout, edges: &Edges) -> Heights {
        let gap = gap(style, Y);
        let count = self.in_flow(node).count();
        let contribution = |child: Node, sizing: Sizing| {
            let heights = self.heights[child.index()];
            let content = heights.resolve(sizing);
            flex::contribution(
                Contribution {
                    style: &self.style[child.index()],
                    edges: &self.edges(child),
                    axis: Y,
                    least: self.least_main(Some(node), child, Y, heights.least),
                    content,
                    measured: self.inherent(child, Y, content),
                },
                false,
                edges.inset[Y],
            )
        };
        let sum = |sizing: Sizing| {
            self.in_flow(node)
                .map(|child| contribution(child, sizing))
                .sum::<f32>()
                + gaps(gap, count)
        };
        let offered = self
            .in_flow(node)
            .map(|child| {
                let child_style = &self.style[child.index()];
                let child_edges = self.edges(child);
                let basis = child_style
                    .basis
                    .fixed()
                    .or(child_style.size.height.fixed())
                    .unwrap_or(self.heights[child.index()].most)
                    .max(child_edges.pad[Y]);
                let floor = child_style
                    .min
                    .height
                    .fixed()
                    .map_or(basis, |min| basis.max(min));
                (floor + child_edges.margin[Y]).max(child_edges.pad[Y])
            })
            .sum::<f32>()
            + gaps(gap, count);
        Heights {
            least: sum(Sizing::Least),
            most: sum(Sizing::Most),
            offered,
        }
    }

    fn row_heights(&self, node: Node, style: &Layout) -> Heights {
        let mut breaks = self.lines[node.index()]
            .iter()
            .skip(1)
            .map(|start| *start as usize)
            .peekable();
        let mut lines = Lines::default();
        for (index, child) in self.in_flow(node).enumerate() {
            while breaks.next_if_eq(&index).is_some() {
                lines.close();
            }
            let heights = self.heights[child.index()];
            let margin = self.edges(child).margin[Y];
            let outer = |content: f32| self.inherent(child, Y, content) + margin;
            lines.reach(Heights {
                least: outer(heights.least),
                most: outer(heights.most),
                offered: outer(heights.offered),
            });
        }
        for _ in breaks {
            lines.close();
        }
        lines.close();
        match style.wrap {
            true => {
                let gaps = gaps(gap(style, Y), lines.count);
                lines.total.map(|height| height + gaps)
            }
            false => {
                let inset = self.edges(node).inset[Y];
                let min = style.min.height.fixed().map(|min| min - inset);
                let max = style.max.height.fixed().map(|max| max - inset);
                lines.first.map(|height| clamp(height, min, max))
            }
        }
    }
}

#[derive(Default)]
struct Lines {
    current: Heights,
    first: Heights,
    total: Heights,
    count: usize,
}

impl Lines {
    fn reach(&mut self, outer: Heights) {
        self.current = Heights {
            least: self.current.least.max(outer.least),
            most: self.current.most.max(outer.most),
            offered: self.current.offered.max(outer.offered),
        };
    }

    fn close(&mut self) {
        let line = std::mem::take(&mut self.current);
        if self.count == 0 {
            self.first = line;
        }
        self.total = Heights {
            least: self.total.least + line.least,
            most: self.total.most + line.most,
            offered: self.total.offered + line.offered,
        };
        self.count += 1;
    }
}

impl Heights {
    fn map(self, each: impl Fn(f32) -> f32) -> Heights {
        Heights {
            least: each(self.least),
            most: each(self.most),
            offered: each(self.offered),
        }
    }
}
