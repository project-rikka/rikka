use crate::scroll::bar;
use crate::style::{Align, Axis, Flow, Justify, Layout};

use super::edges::{Edges, X, Y, clamp, gap, gaps, self_align, sized, stretches};
use super::flex::{self, Item};
use super::{ARRANGED, BELOW, DIRTY, Layouts, Node, PLACED, TOUCHED};
use crate::runtime::scratch::Scratch;

impl Layouts {
    pub(super) fn place(&mut self, node: Node, width: f32, height: f32) {
        let at = node.index();
        if !self.visits(node) && self.laid[at].height == height && self.size[at][X] == width {
            return;
        }
        self.flags[at] &= !(DIRTY | BELOW | TOUCHED);
        self.flags[at] |= PLACED | ARRANGED;
        self.laid[at].height = height;
        self.want[at][Y] = height;
        let height = self.settle(node, Y, height);
        self.size[at] = [width, height];
        let edges = self.edges(node);
        if self.children[at].is_empty() {
            let measured = match self.fits[at] {
                Some((_, size)) if self.leaf[at].is_some() => size,
                _ => rikka_types::Size::ZERO,
            };
            self.content[at] = [
                measured.width + edges.pad[X],
                measured.height + edges.pad[Y],
            ];
            return;
        }
        let style = self.style[at];
        let flowing = match style.flow {
            Flow::Column => self.place_column(node, &style, &edges),
            Flow::Row => self.place_row(node, &style, &edges),
            Flow::Grid(tracks) => self.place_grid(node, &style, &edges, tracks),
        };
        let mut content = [0.0_f32; 2];
        for &(child, [child_width, child_height]) in flowing.iter() {
            self.place(child, child_width, child_height);
            let area = self.area[child.index()];
            content = reach(content, self.contribution(child, area));
        }
        if !matches!(style.flow, Flow::Grid(_)) {
            content[X] += edges.end[X];
            content[Y] += edges.end[Y];
        }
        let area = [width - edges.gutter[X], height - edges.gutter[Y]];
        let placed: Scratch<Node> = self.placed_out(node).collect();
        for &child in placed.iter() {
            let child_height = self.placed_height(child, area[Y]);
            let child_style = self.style[child.index()];
            let origin = child_style.inset.expect("node is out of flow");
            let margin = self.edges(child).margin_start;
            self.at[child.index()] = [origin.x + margin[X], origin.y + margin[Y]];
            let child_width = self.size[child.index()][X];
            self.place(child, child_width, child_height);
            let contribution = match style.flow {
                Flow::Grid(_) => self.contribution(child, [0.0, 0.0]),
                Flow::Row | Flow::Column => self.placed_contribution(child),
            };
            content = reach(content, contribution);
        }
        self.content[at] = content;
        if style.scrolls.is_some() {
            self.settle_lane(node, &style);
        }
    }

    fn visible(&self, child: Node) -> Option<[f32; 2]> {
        let at = child.index();
        let style = &self.style[at];
        let size = self.size[at];
        let content = self.content[at];
        let visible = |axis: usize| match super::edges::scrolls(style, axis) {
            true => size[axis],
            false => size[axis].max(content[axis]),
        };
        let reach = [visible(X), visible(Y)];
        (reach[X] > 0.0 && reach[Y] > 0.0).then_some(reach)
    }

    fn contribution(&self, child: Node, area: [f32; 2]) -> [f32; 2] {
        let Some(reach) = self.visible(child) else {
            return [0.0, 0.0];
        };
        let at = child.index();
        let origin = [self.at[at][X] - area[X], self.at[at][Y] - area[Y]];
        [
            (origin[X] + reach[X]).max(0.0) - origin[X].min(0.0),
            (origin[Y] + reach[Y]).max(0.0) - origin[Y].min(0.0),
        ]
    }

    fn placed_contribution(&self, child: Node) -> [f32; 2] {
        let Some(reach) = self.visible(child) else {
            return [0.0, 0.0];
        };
        let origin = self.at[child.index()];
        [origin[X] + reach[X], origin[Y] + reach[Y]]
    }

    pub(super) fn placed_height(&self, child: Node, area: f32) -> f32 {
        let style = &self.style[child.index()];
        let edges = self.edges(child);
        let room = Some(area);
        let min = Some(
            sized(style.min.height, room)
                .unwrap_or(edges.pad[Y])
                .max(edges.pad[Y]),
        );
        let max = sized(style.max.height, room);
        match sized(style.size.height, room) {
            Some(own) => clamp(own, min, max),
            None => clamp(self.heights[child.index()].offered, min, max),
        }
    }

    fn place_column(
        &mut self,
        node: Node,
        style: &Layout,
        edges: &Edges,
    ) -> Scratch<(Node, [f32; 2])> {
        let [width, height] = self.size[node.index()];
        let inner = [width - edges.inset[X], height - edges.inset[Y]];
        let gap = gap(style, Y);
        let children: Scratch<Node> = self.in_flow(node).collect();
        let mut items: Scratch<Item> = children
            .iter()
            .map(|child| {
                let heights = self.heights[child.index()];
                Item::new(
                    &self.style[child.index()],
                    &self.edges(*child),
                    Y,
                    Some(inner[Y]),
                    heights.most,
                    self.least_main(Some(node), *child, Y, heights.least),
                )
            })
            .collect();
        let room = flex::room(&items, inner[Y], gap, super::edges::scrolls(style, Y));
        flex::resolve(&mut items, room, gap);
        let used: f32 = items
            .iter()
            .map(|item| item.target + item.margin)
            .sum::<f32>()
            + gaps(gap, items.len());
        let free = room.max(0.0) - used;
        let mut main = edges.start[Y];
        let mut laid = Scratch::new();
        for (index, (&child, item)) in children.iter().zip(items.iter()).enumerate() {
            main += justified(style.justify, free, gap, index == 0);
            let child_edges = self.edges(child);
            let child_width = self.size[child.index()][X];
            let across = inner[X].max(0.0) - (self.want[child.index()][X] + child_edges.margin[X]);
            let cross = aligned(self_align(&self.style[child.index()], style), across);
            self.area[child.index()] = [0.0, 0.0];
            self.at[child.index()] = [
                edges.start[X] + cross + child_edges.margin_start[X],
                main + child_edges.margin_start[Y],
            ];
            main += self.settle(child, Y, item.target) + item.margin;
            laid.push((child, [child_width, item.target]));
        }
        laid
    }

    fn place_row(
        &mut self,
        node: Node,
        style: &Layout,
        edges: &Edges,
    ) -> Scratch<(Node, [f32; 2])> {
        let [width, height] = self.size[node.index()];
        let inner = [width - edges.inset[X], height - edges.inset[Y]];
        let room = Some(inner[Y]);
        let children: Scratch<Node> = self.in_flow(node).collect();
        let mut bounds: Scratch<usize> = self.lines[node.index()]
            .iter()
            .map(|start| *start as usize)
            .collect();
        if bounds.is_empty() {
            bounds.push(0);
        }
        bounds.push(children.len());
        let hypothetical = |layouts: &Self, child: Node| -> f32 {
            let child_style = &layouts.style[child.index()];
            let child_edges = layouts.edges(child);
            let min = sized(child_style.min.height, room);
            let max = sized(child_style.max.height, room);
            match sized(child_style.size.height, room) {
                Some(own) => clamp(own, min, max).max(child_edges.pad[Y]),
                None => {
                    clamp(layouts.heights[child.index()].offered, min, max).max(child_edges.pad[Y])
                }
            }
        };
        let mut lines: Scratch<f32> = match style.wrap {
            false => Scratch::filled(inner[Y].max(0.0), 1),
            true => bounds
                .windows(2)
                .map(|pair| {
                    children[pair[0]..pair[1]]
                        .iter()
                        .map(|child| hypothetical(self, *child) + self.edges(*child).margin[Y])
                        .fold(0.0, f32::max)
                })
                .collect(),
        };
        let cross_gap = gap(style, Y);
        if style.wrap {
            let total = lines.iter().sum::<f32>() + gaps(cross_gap, lines.len());
            let room = inner[Y].max(0.0);
            if total < room {
                let addition = (room - total) / lines.len() as f32;
                lines.iter_mut().for_each(|line| *line += addition);
            }
        }
        let gap = gap(style, X);
        let mut laid = Scratch::new();
        let mut line_start = edges.start[Y];
        for (number, pair) in bounds.windows(2).enumerate() {
            let line = lines[number];
            if number > 0 {
                line_start += cross_gap;
            }
            let members = &children[pair[0]..pair[1]];
            let used: f32 = members
                .iter()
                .map(|child| self.want[child.index()][X] + self.edges(*child).margin[X])
                .sum::<f32>()
                + gaps(gap, members.len());
            let free = inner[X].max(0.0) - used;
            let mut main = edges.start[X];
            for (index, child) in members.iter().enumerate() {
                main += justified(style.justify, free, gap, index == 0);
                let child_style = self.style[child.index()];
                let child_edges = self.edges(*child);
                let child_width = self.size[child.index()][X];
                let child_height = match stretches(&child_style, style, Y) {
                    true => clamp(
                        line - child_edges.margin[Y],
                        sized(child_style.min.height, room),
                        sized(child_style.max.height, room),
                    ),
                    false => hypothetical(self, *child),
                };
                let across = line - (child_height + child_edges.margin[Y]);
                let cross = aligned(self_align(&child_style, style), across);
                self.area[child.index()] = [0.0, 0.0];
                self.at[child.index()] = [
                    main + child_edges.margin_start[X],
                    line_start + cross + child_edges.margin_start[Y],
                ];
                main += child_width + child_edges.margin[X];
                laid.push((*child, [child_width, child_height]));
            }
            line_start += line;
        }
        laid
    }

    fn settle_lane(&mut self, node: Node, style: &Layout) {
        let at = node.index();
        let Some(along) = style.scrolls else {
            return;
        };
        let axis = match along {
            Axis::Horizontal => X,
            Axis::Vertical => Y,
        };
        let content = self.length[at].unwrap_or(self.content[at][axis]);
        let room = self.size[at][axis];
        let lane_needed = match bar::furthest(room, content) > 0.0 {
            true => bar::lane(self.kept_clear(node, along)),
            false => 0.0,
        };
        if lane_needed != self.lane[at] {
            self.lane[at] = lane_needed;
            self.adopt_style(at);
            self.mark(node);
            self.moved.push(node);
            self.again = true;
        }
    }

    fn kept_clear(&self, node: Node, along: Axis) -> f32 {
        let extent_of = |layout: &Layout| match along {
            Axis::Vertical => layout.padding.right(),
            Axis::Horizontal => layout.padding.bottom(),
        };
        let own = extent_of(&self.style[node.index()]);
        match self.children[node.index()].as_slice() {
            [only] => own + extent_of(&self.style[only.index()]),
            _ => own,
        }
    }
}

fn reach(content: [f32; 2], contribution: [f32; 2]) -> [f32; 2] {
    [
        content[X].max(contribution[X]),
        content[Y].max(contribution[Y]),
    ]
}

pub(super) fn justified(justify: Option<Justify>, free: f32, gap: f32, first: bool) -> f32 {
    match first {
        false => gap,
        true => match justify.unwrap_or(Justify::Start) {
            Justify::Start => 0.0,
            Justify::Center => free / 2.0,
            Justify::End => free,
        },
    }
}

pub(super) fn aligned(align: Align, free: f32) -> f32 {
    match align {
        Align::Start | Align::Stretch => 0.0,
        Align::Center => free / 2.0,
        Align::End => free,
    }
}
