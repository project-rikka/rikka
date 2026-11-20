use crate::style::{Flow, Layout};

use super::edges::{Edges, X, clamp, gap, gaps, length, sized, stretches};
use super::flex::{self, Contribution, Item};
use super::{BELOW, DIRTY, Layouts, Node, PLACED_OUT, TOUCHED};
use crate::runtime::scratch::Scratch;

impl Layouts {
    pub(super) fn edges(&self, node: Node) -> Edges {
        self.edges[node.index()]
    }

    pub(super) fn in_flow(&self, node: Node) -> impl Iterator<Item = Node> + '_ {
        self.children[node.index()]
            .iter()
            .copied()
            .filter(|child| self.flags[child.index()] & PLACED_OUT == 0)
    }

    pub(super) fn placed_out(&self, node: Node) -> impl Iterator<Item = Node> + '_ {
        self.children[node.index()]
            .iter()
            .copied()
            .filter(|child| self.flags[child.index()] & PLACED_OUT != 0)
    }

    pub(super) fn least_main(
        &self,
        parent: Option<Node>,
        child: Node,
        axis: usize,
        content: f32,
    ) -> f32 {
        let overflows =
            parent.is_some_and(|parent| super::edges::scrolls(&self.style[parent.index()], axis));
        match overflows || self.children[child.index()].is_empty() {
            true => content,
            false => 0.0,
        }
    }

    pub(super) fn inherent(&self, node: Node, axis: usize, content: f32) -> f32 {
        let style = &self.style[node.index()];
        let pb = self.edges(node).pad[axis];
        let min = length(style.min, axis).fixed();
        let max = length(style.max, axis).fixed();
        match length(style.size, axis).fixed() {
            Some(own) => clamp(own, min, max).max(pb),
            None => clamp(content, min, max).max(pb),
        }
    }

    pub(super) fn intrinsic(&mut self, node: Node) {
        let at = node.index();
        if self.flags[at] & (DIRTY | BELOW) == 0 {
            return;
        }
        for index in 0..self.children[at].len() {
            let child = self.children[at][index];
            self.intrinsic(child);
        }
        self.widths[at] = self.content_widths(node);
    }

    fn content_widths(&mut self, node: Node) -> [f32; 2] {
        let at = node.index();
        let style = self.style[at];
        let edges = self.edges(node);
        if self.children[at].is_empty() {
            let pad = edges.pad[X];
            return match self.leaf[at].as_mut() {
                Some(leaf) => {
                    let widths = leaf.widths();
                    self.natural[at] = widths.natural;
                    [widths.least + pad, widths.most + pad]
                }
                None => [pad, pad],
            };
        }
        let [least, most] = match style.flow {
            Flow::Row => self.row_widths(node, &style, &edges),
            Flow::Column => self.column_widths(node),
            Flow::Grid(tracks) => self.grid_widths(node, &style, tracks),
        };
        let min = style.min.width.fixed();
        let max = style.max.width.fixed();
        let inset = edges.inset[X];
        [
            clamp(least + inset, min, max).max(edges.pad[X]),
            clamp(most + inset, min, max).max(edges.pad[X]),
        ]
    }

    fn row_widths(&self, node: Node, style: &Layout, edges: &Edges) -> [f32; 2] {
        let gap = gap(style, X);
        let children: Scratch<Node> = self.in_flow(node).collect();
        let contribution = |child: Node, least: bool| {
            let [low, high] = self.widths[child.index()];
            let content = if least { low } else { high };
            flex::contribution(
                Contribution {
                    style: &self.style[child.index()],
                    edges: &self.edges(child),
                    axis: X,
                    least: self.least_main(Some(node), child, X, low),
                    content,
                    measured: self.inherent(child, X, content),
                },
                true,
                edges.inset[X],
            )
        };
        let floor = |child: Node| {
            let child_style = &self.style[child.index()];
            let child_edges = self.edges(child);
            let basis = child_style
                .basis
                .fixed()
                .or(child_style.size.width.fixed())
                .unwrap_or(self.widths[child.index()][0])
                .max(child_edges.pad[X]);
            let floor = child_style
                .min
                .width
                .fixed()
                .map_or(basis, |min| basis.max(min));
            (floor + child_edges.margin[X]).max(child_edges.pad[X])
        };
        let limit = match (style.wrap, style.max.width.fixed()) {
            (true, Some(max)) => Some(style.min.width.fixed().map_or(max, |min| max.max(min))),
            _ => None,
        };
        let lines = |least: bool| -> Scratch<usize> {
            let Some(limit) = limit else {
                return match (style.wrap, least) {
                    (true, true) => (0..=children.len()).collect(),
                    _ => flex::unbroken(children.len()),
                };
            };
            let outer: Scratch<f32> = children
                .iter()
                .map(|child| {
                    let [low, high] = self.widths[child.index()];
                    let content = if least { low } else { high };
                    Item::new(
                        &self.style[child.index()],
                        &self.edges(*child),
                        X,
                        None,
                        content,
                        self.least_main(Some(node), *child, X, low),
                    )
                    .outer()
                })
                .collect();
            flex::broken(&outer, limit, gap)
        };
        let most = lines(false)
            .windows(2)
            .map(|pair| {
                let line = &children[pair[0]..pair[1]];
                line.iter()
                    .map(|child| contribution(*child, false))
                    .sum::<f32>()
                    + gaps(gap, line.len())
            })
            .fold(0.0, f32::max);
        let least = match style.wrap {
            false => {
                children
                    .iter()
                    .map(|child| contribution(*child, true))
                    .sum::<f32>()
                    + gaps(gap, children.len())
            }
            true => lines(true)
                .windows(2)
                .map(|pair| {
                    let line = &children[pair[0]..pair[1]];
                    line.iter().map(|child| floor(*child)).sum::<f32>() + gaps(gap, line.len())
                })
                .fold(0.0, f32::max),
        };
        [least, most]
    }

    fn column_widths(&self, node: Node) -> [f32; 2] {
        let mut widths = [0.0_f32; 2];
        for child in self.in_flow(node) {
            let edges = self.edges(child);
            for (slot, content) in widths.iter_mut().zip(self.widths[child.index()]) {
                *slot = slot.max(self.inherent(child, X, content) + edges.margin[X]);
            }
        }
        widths
    }

    pub(super) fn lay_width(&mut self, node: Node, width: f32) {
        let at = node.index();
        if !self.visits(node) && self.laid[at].width == width {
            return;
        }
        self.flags[at] |= TOUCHED;
        self.laid[at].width = width;
        self.want[at][X] = width;
        let width = self.settle(node, X, width);
        self.size[at][X] = width;
        if self.children[at].is_empty() {
            return;
        }
        let style = self.style[at];
        let edges = self.edges(node);
        let inner = width - edges.inset[X];
        let widths = match style.flow {
            Flow::Row => self.row_children(node, &style, inner),
            Flow::Column => self.column_children(node, &style, inner),
            Flow::Grid(tracks) => self.grid_children(node, &style, tracks, inner),
        };
        let area = width - edges.gutter[X];
        let placed: Scratch<Node> = self.placed_out(node).collect();
        let placed: Scratch<(Node, f32)> = placed
            .iter()
            .map(|&child| (child, self.placed_width(child, width, area)))
            .collect();
        for &(child, width) in widths.iter().chain(placed.iter()) {
            self.lay_width(child, width);
        }
    }

    fn row_children(&mut self, node: Node, style: &Layout, inner: f32) -> Scratch<(Node, f32)> {
        let gap = gap(style, X);
        let children: Scratch<Node> = self.in_flow(node).collect();
        let mut items: Scratch<Item> = children
            .iter()
            .map(|child| {
                let [least, most] = self.widths[child.index()];
                Item::new(
                    &self.style[child.index()],
                    &self.edges(*child),
                    X,
                    Some(inner),
                    most,
                    self.least_main(Some(node), *child, X, least),
                )
            })
            .collect();
        let bounds = match style.wrap {
            true => {
                let outer: Scratch<f32> = items.iter().map(Item::outer).collect();
                flex::broken(&outer, inner, gap)
            }
            false => flex::unbroken(items.len()),
        };
        let scrolls = super::edges::scrolls(style, X);
        for pair in bounds.windows(2) {
            let line = &mut items[pair[0]..pair[1]];
            let room = flex::room(line, inner, gap, scrolls);
            flex::resolve(line, room, gap);
        }
        let lines = &mut self.lines[node.index()];
        lines.clear();
        lines.extend(bounds[..bounds.len() - 1].iter().map(|start| *start as u32));
        children
            .iter()
            .zip(items.iter())
            .map(|(child, item)| (*child, item.target))
            .collect()
    }

    fn column_children(&mut self, node: Node, style: &Layout, inner: f32) -> Scratch<(Node, f32)> {
        let children: Scratch<Node> = self.in_flow(node).collect();
        children
            .iter()
            .map(|&child| (child, self.cross_width(child, style, inner)))
            .collect()
    }

    pub(super) fn cross_width(&mut self, child: Node, parent: &Layout, inner: f32) -> f32 {
        let style = self.style[child.index()];
        let edges = self.edges(child);
        let room = Some(inner);
        let min = sized(style.min.width, room);
        let max = sized(style.max.width, room);
        if stretches(&style, parent, X) {
            return clamp(inner - edges.margin[X], min, max);
        }
        match sized(style.size.width, room) {
            Some(own) => clamp(own, min, max).max(edges.pad[X]),
            None => {
                let available = clamp(inner, min, max).max(edges.pad[X]);
                let offered = self.offered_width(child, available, room);
                clamp(offered, min, max).max(edges.pad[X])
            }
        }
    }

    pub(super) fn placed_width(&mut self, child: Node, container: f32, area: f32) -> f32 {
        let style = self.style[child.index()];
        let edges = self.edges(child);
        let room = Some(area);
        let min = Some(
            sized(style.min.width, room)
                .unwrap_or(edges.pad[X])
                .max(edges.pad[X]),
        );
        let max = sized(style.max.width, room);
        match sized(style.size.width, room) {
            Some(own) => clamp(own, min, max),
            None => {
                let available = clamp(container, min, max);
                clamp(self.offered_width(child, available, room), min, max)
            }
        }
    }

    pub(super) fn offered_width(&mut self, node: Node, available: f32, room: Option<f32>) -> f32 {
        let at = node.index();
        let style = self.style[at];
        let edges = self.edges(node);
        let available = available - edges.margin[X];
        if self.children[at].is_empty() {
            let pad = edges.pad[X];
            return match self.leaf[at].as_mut() {
                Some(leaf) => {
                    let query = available - pad;
                    let size = leaf.size_for_width(query);
                    self.fits[at] = Some((query, size));
                    size.width + pad
                }
                None => pad,
            };
        }
        let inner = available - edges.inset[X];
        let content = match style.flow {
            Flow::Row => self.offered_row(node, &style, &edges, inner),
            Flow::Column => {
                let children: Scratch<Node> = self.in_flow(node).collect();
                let mut widest = 0.0_f32;
                for &child in children.iter() {
                    let child_style = self.style[child.index()];
                    let child_edges = self.edges(child);
                    let min = child_style.min.width.fixed();
                    let max = child_style.max.width.fixed();
                    let width = match child_style.size.width.fixed() {
                        Some(own) => clamp(own, min, max).max(child_edges.pad[X]),
                        None => {
                            let offered = clamp(inner, min, max).max(child_edges.pad[X]);
                            let width = self.offered_width(child, offered, None);
                            clamp(width, min, max).max(child_edges.pad[X])
                        }
                    };
                    widest = widest.max(width + child_edges.margin[X]);
                }
                widest + edges.inset[X]
            }
            Flow::Grid(_) => {
                let [least, most] = self.widths[at];
                clamp(available, Some(least), Some(most))
            }
        };
        let min = sized(style.min.width, room);
        let max = sized(style.max.width, room);
        clamp(content, min, max).max(edges.pad[X])
    }

    fn offered_row(&self, node: Node, style: &Layout, edges: &Edges, inner: f32) -> f32 {
        let gap = gap(style, X);
        let items: Scratch<Item> = self
            .in_flow(node)
            .map(|child| {
                let [least, most] = self.widths[child.index()];
                Item::new(
                    &self.style[child.index()],
                    &self.edges(child),
                    X,
                    None,
                    most,
                    self.least_main(Some(node), child, X, least),
                )
            })
            .collect();
        let bounds = match style.wrap {
            true => {
                let outer: Scratch<f32> = items.iter().map(Item::outer).collect();
                flex::broken(&outer, inner, gap)
            }
            false => flex::unbroken(items.len()),
        };
        let longest = bounds
            .windows(2)
            .map(|pair| {
                items[pair[0]..pair[1]].iter().map(Item::outer).sum::<f32>()
                    + gaps(gap, pair[1] - pair[0])
            })
            .fold(0.0, f32::max);
        let size = longest + edges.inset[X];
        match bounds.len() > 2 {
            true => size.max(inner),
            false => size,
        }
    }
}
