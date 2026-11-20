mod boundary;
mod edges;
mod flex;
mod grid;
mod heights;
mod leaf;
mod place;
mod round;
mod standalone;
mod widths;

use rikka_types::{Point, Rect, Size};

use crate::paint::MeasureSpec;
use crate::style::{Axis, Layout, Length};

pub(crate) use leaf::Leaf;
pub use standalone::LayoutTree;

use edges::{X, Y};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Node(u32);

impl Node {
    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }

    pub(crate) fn slot(self) -> u32 {
        self.0
    }

    pub(crate) fn from_slot(slot: u32) -> Self {
        Node(slot)
    }
}

const NO_PARENT: u32 = u32::MAX;

const DIRTY: u8 = 1;
const BELOW: u8 = 1 << 1;
const TOUCHED: u8 = 1 << 2;
const PLACED: u8 = 1 << 3;
const ARRANGED: u8 = 1 << 4;
const QUEUED: u8 = 1 << 5;
const PLACED_OUT: u8 = 1 << 6;

const SETTLE: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Fix {
    Pinned {
        at: Point,
        width: Option<f32>,
        height: Option<f32>,
        max_height: Option<f32>,
    },
    Retained(Rect),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Heights {
    least: f32,
    most: f32,
    offered: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct AvailableSize {
    width: f32,
    height: f32,
}

impl AvailableSize {
    const NEVER: Self = Self {
        width: f32::NAN,
        height: f32::NAN,
    };
}

#[derive(Default)]
pub(crate) struct Layouts {
    parent: Vec<u32>,
    children: Vec<Vec<Node>>,
    free: Vec<u32>,
    declared: Vec<Layout>,
    fix: Vec<Option<Fix>>,
    length: Vec<Option<f32>>,
    style: Vec<Layout>,
    edges: Vec<edges::Edges>,
    leaf: Vec<Option<Leaf>>,
    lane: Vec<f32>,
    flags: Vec<u8>,
    widths: Vec<[f32; 2]>,
    fits: Vec<Option<(f32, Size)>>,
    natural: Vec<Size>,
    tracks: Vec<Vec<f32>>,
    heights: Vec<Heights>,
    lines: Vec<Vec<u32>>,
    size: Vec<[f32; 2]>,
    want: Vec<[f32; 2]>,
    at: Vec<[f32; 2]>,
    area: Vec<[f32; 2]>,
    content: Vec<[f32; 2]>,
    laid: Vec<AvailableSize>,
    world: Vec<[f32; 2]>,
    rect: Vec<Rect>,
    shown: Vec<Size>,
    moved: Vec<Node>,
    boundaries: Vec<Node>,
    again: bool,
}

impl Layouts {
    pub(crate) fn create(&mut self, layout: Layout, measure: Option<MeasureSpec>) -> Node {
        let leaf = measure.map(Leaf::new);
        let node = match self.free.pop() {
            Some(slot) => {
                let at = slot as usize;
                self.parent[at] = NO_PARENT;
                self.children[at].clear();
                self.declared[at] = layout;
                self.fix[at] = None;
                self.length[at] = None;
                self.style[at] = layout;
                self.leaf[at] = leaf;
                self.lane[at] = 0.0;
                self.flags[at] = 0;
                self.widths[at] = [0.0; 2];
                self.fits[at] = None;
                self.natural[at] = Size::ZERO;
                self.tracks[at].clear();
                self.heights[at] = Heights::default();
                self.lines[at].clear();
                self.size[at] = [0.0; 2];
                self.want[at] = [0.0; 2];
                self.at[at] = [0.0; 2];
                self.area[at] = [0.0; 2];
                self.content[at] = [0.0; 2];
                self.laid[at] = AvailableSize::NEVER;
                self.world[at] = [f32::NAN; 2];
                self.rect[at] = Rect::ZERO;
                self.shown[at] = Size::ZERO;
                Node(slot)
            }
            None => {
                let slot = u32::try_from(self.parent.len()).expect("layout node count exceeds u32");
                self.parent.push(NO_PARENT);
                self.children.push(Vec::new());
                self.declared.push(layout);
                self.fix.push(None);
                self.length.push(None);
                self.style.push(layout);
                self.edges.push(edges::Edges::default());
                self.leaf.push(leaf);
                self.lane.push(0.0);
                self.flags.push(0);
                self.widths.push([0.0; 2]);
                self.fits.push(None);
                self.natural.push(Size::ZERO);
                self.tracks.push(Vec::new());
                self.heights.push(Heights::default());
                self.lines.push(Vec::new());
                self.size.push([0.0; 2]);
                self.want.push([0.0; 2]);
                self.at.push([0.0; 2]);
                self.area.push([0.0; 2]);
                self.content.push([0.0; 2]);
                self.laid.push(AvailableSize::NEVER);
                self.world.push([f32::NAN; 2]);
                self.rect.push(Rect::ZERO);
                self.shown.push(Size::ZERO);
                Node(slot)
            }
        };
        self.adopt_style(node.index());
        self.refresh(node);
        self.mark(node);
        node
    }

    pub(crate) fn remove(&mut self, node: Node) {
        let parent = self.parent[node.index()];
        if parent != NO_PARENT {
            self.children[parent as usize].retain(|child| *child != node);
            self.mark_contents(Node(parent));
        }
        self.free_tree(node);
    }

    fn free_tree(&mut self, node: Node) {
        let at = node.index();
        for index in 0..self.children[at].len() {
            let child = self.children[at][index];
            if self.parent[child.index()] == node.0 {
                self.free_tree(child);
            }
        }
        self.children[at].clear();
        self.parent[at] = NO_PARENT;
        self.leaf[at] = None;
        self.free.push(node.0);
    }

    pub(crate) fn set_children(&mut self, node: Node, children: &[Node]) {
        let at = node.index();
        if self.children[at] == children {
            return;
        }
        let mut detached = std::mem::take(&mut self.children[at]);
        for child in detached.drain(..) {
            if self.parent[child.index()] == node.0 {
                self.parent[child.index()] = NO_PARENT;
            }
        }
        for child in children {
            let was = self.parent[child.index()];
            if was != NO_PARENT && was != node.0 {
                self.children[was as usize].retain(|other| other != child);
                self.mark_contents(Node(was));
            }
            self.parent[child.index()] = node.0;
        }
        detached.extend_from_slice(children);
        self.children[at] = detached;
        self.mark_contents(node);
    }

    pub(crate) fn set_style(&mut self, node: Node, layout: Layout) {
        let at = node.index();
        self.declared[at] = layout;
        if matches!(self.fix[at], Some(Fix::Pinned { .. })) {
            self.fix[at] = None;
        }
        self.refresh(node);
    }

    pub(crate) fn set_measure(&mut self, node: Node, measure: Option<MeasureSpec>) {
        let at = node.index();
        match (&mut self.leaf[at], measure) {
            (Some(leaf), Some(spec)) => leaf.respec(spec),
            (slot, measure) => *slot = measure.map(Leaf::new),
        }
        self.fits[at] = None;
        self.mark(node);
    }

    pub(crate) fn refit(&mut self, node: Node) {
        self.fits[node.index()] = None;
        self.mark(node);
    }

    pub(crate) fn leaf(&self, node: Node) -> Option<&Leaf> {
        self.leaf[node.index()].as_ref()
    }

    pub(crate) fn leaf_mut(&mut self, node: Node) -> Option<&mut Leaf> {
        self.leaf[node.index()].as_mut()
    }

    pub(crate) fn set_length(&mut self, node: Node, length: Option<f32>) {
        let at = node.index();
        if self.length[at] == length {
            return;
        }
        self.length[at] = length;
        self.refresh(node);
        self.mark(node);
    }

    pub(crate) fn fix(&mut self, node: Node, fix: Fix) {
        let at = node.index();
        if self.fix[at] == Some(fix) {
            return;
        }
        self.fix[at] = Some(fix);
        self.refresh(node);
    }

    pub(crate) fn release(&mut self, node: Node) {
        if self.fix[node.index()].take().is_some() {
            self.refresh(node);
        }
    }

    pub(crate) fn fixed(&self, node: Node) -> Option<Fix> {
        self.fix[node.index()]
    }

    pub(crate) fn take_arranged(&mut self, node: Node) -> bool {
        let arranged = self.flags[node.index()] & ARRANGED != 0;
        self.flags[node.index()] &= !ARRANGED;
        arranged
    }

    pub(crate) fn is_dirty(&self, node: Node) -> bool {
        self.flags[node.index()] & (DIRTY | BELOW) != 0
    }

    pub(crate) fn needs_solve(&self, root: Node) -> bool {
        self.is_dirty(root) || !self.boundaries.is_empty()
    }

    pub(crate) fn rect(&self, node: Node) -> Rect {
        self.rect[node.index()]
    }

    pub(crate) fn solved_width(&self, node: Node) -> f32 {
        self.size[node.index()][0]
    }

    pub(crate) fn was_laid(&self, node: Node) -> bool {
        !self.world[node.index()][0].is_nan()
    }

    pub(crate) fn content(&self, node: Node) -> Size {
        self.shown[node.index()]
    }

    pub(crate) fn lane(&self, node: Node) -> f32 {
        self.lane[node.index()]
    }

    pub(crate) fn child_rect(&self, node: Node, index: usize) -> Option<Rect> {
        let child = self.children[node.index()].get(index)?;
        Some(self.rect(*child))
    }

    pub(crate) fn children(&self, node: Node) -> &[Node] {
        &self.children[node.index()]
    }

    pub(crate) fn parent(&self, node: Node) -> Option<Node> {
        let parent = self.parent[node.index()];
        (parent != NO_PARENT).then_some(Node(parent))
    }

    pub(crate) fn lineage(&self, node: Node, into: &mut Vec<Node>) {
        let from = into.len();
        let mut at = Some(node);
        while let Some(step) = at {
            into.push(step);
            at = self.parent(step);
        }
        into[from..].reverse();
    }

    pub(crate) fn take_moved(&mut self, into: &mut Vec<Node>) {
        into.clear();
        std::mem::swap(&mut self.moved, into);
    }

    fn refresh(&mut self, node: Node) {
        let at = node.index();
        let mut style = self.declared[at];
        match self.fix[at] {
            Some(Fix::Pinned {
                at: origin,
                width,
                height,
                max_height,
            }) => {
                style.inset = Some(origin);
                if let Some(width) = width {
                    style.size.width = Length::px(width);
                }
                if let Some(height) = height {
                    style.size.height = Length::px(height);
                }
                style.max.height = max_height.map_or(Length::HUG, Length::px);
            }
            Some(Fix::Retained(rect)) => {
                style.inset = Some(rect.origin);
                style.size.width = Length::px(rect.width());
                style.size.height = Length::px(rect.height());
            }
            None => {}
        }
        if let Some(along) = style.scrolls {
            let axis = match along {
                Axis::Horizontal => X,
                Axis::Vertical => Y,
            };
            let hugs = edges::length(style.size, axis) == Length::HUG && style.grow == 0.0;
            let least = match (hugs, self.length[at]) {
                (true, Some(content)) => {
                    let most = edges::length(style.max, axis)
                        .fixed()
                        .unwrap_or(f32::INFINITY);
                    (content + along.inset(style.padding)).min(most)
                }
                _ => 0.0,
            };
            match along {
                Axis::Horizontal => style.min.width = Length::px(least),
                Axis::Vertical => style.min.height = Length::px(least),
            }
        }
        if self.style[at] != style {
            self.style[at] = style;
            self.adopt_style(at);
            self.mark(node);
        }
    }

    fn adopt_style(&mut self, at: usize) {
        self.edges[at] = edges::Edges::new(&self.style[at], self.lane[at]);
        match self.style[at].inset.is_some() {
            true => self.flags[at] |= PLACED_OUT,
            false => self.flags[at] &= !PLACED_OUT,
        }
    }

    fn mark(&mut self, node: Node) {
        self.flags[node.index()] |= DIRTY;
        self.climb(self.parent[node.index()]);
    }

    fn mark_contents(&mut self, node: Node) {
        self.flags[node.index()] |= DIRTY;
        match self.is_boundary(node) {
            true => self.queue_boundary(node),
            false => self.climb(self.parent[node.index()]),
        }
    }

    fn climb(&mut self, from: u32) {
        let mut parent = from;
        while parent != NO_PARENT && self.flags[parent as usize] & BELOW == 0 {
            self.flags[parent as usize] |= BELOW;
            if self.is_boundary(Node(parent)) {
                self.queue_boundary(Node(parent));
                return;
            }
            parent = self.parent[parent as usize];
        }
    }

    fn visits(&self, node: Node) -> bool {
        self.flags[node.index()] & (DIRTY | BELOW | TOUCHED) != 0
    }

    pub(crate) fn solve(&mut self, root: Node, viewport: Size, measured: bool) {
        for _ in 0..SETTLE {
            self.again = false;
            self.settle_boundaries();
            self.intrinsic(root);
            let placed = self.style[root.index()].inset;
            let width = match placed {
                Some(_) => self.placed_width(root, viewport.width, viewport.width),
                None => self.root_width(root, viewport),
            };
            self.lay_width(root, width);
            self.measure_heights(root);
            let margin = self.style[root.index()].margin;
            let height = match placed {
                Some(origin) => {
                    self.at[root.index()] = [origin.x + margin.left(), origin.y + margin.top()];
                    self.placed_height(root, viewport.height)
                }
                None => {
                    let (height, cross) = self.root_height(root, viewport, measured);
                    self.at[root.index()] = [margin.left(), margin.top() + cross];
                    height
                }
            };
            self.place(root, self.size[root.index()][X], height);
            if !self.again {
                break;
            }
        }
        self.round(root, [0.0, 0.0]);
    }

    fn settle(&self, node: Node, axis: usize, value: f32) -> f32 {
        let style = &self.style[node.index()];
        let floors = self.children[node.index()].is_empty()
            || matches!(style.flow, crate::style::Flow::Grid(_))
            || edges::main_axis(style) != axis;
        match floors {
            true => value.max(edges::Edges::new(style, self.lane[node.index()]).pad[axis]),
            false => value,
        }
    }

    fn root_width(&self, root: Node, viewport: Size) -> f32 {
        let style = &self.style[root.index()];
        let edges = edges::Edges::new(style, self.lane[root.index()]);
        let room = Some(viewport.width);
        let [least, most] = self.widths[root.index()];
        let item = flex::Item::new(
            style,
            &edges,
            X,
            room,
            most,
            self.least_main(None, root, X, least),
        );
        let mut items = [item];
        flex::resolve(&mut items, viewport.width, 0.0);
        items[0].target
    }

    fn root_height(&self, root: Node, viewport: Size, measured: bool) -> (f32, f32) {
        let style = &self.style[root.index()];
        let edges = edges::Edges::new(style, self.lane[root.index()]);
        let room = Some(viewport.height);
        let min = edges::sized(style.min.height, room);
        let max = edges::sized(style.max.height, room);
        let line = viewport.height;
        let align = style.align_self.unwrap_or(match measured {
            true => crate::style::Align::Start,
            false => crate::style::Align::Stretch,
        });
        let stretched = align == crate::style::Align::Stretch && style.size.height == Length::HUG;
        let height = match stretched {
            true => edges::clamp(line - edges.margin[Y], min, max),
            false => match edges::sized(style.size.height, room) {
                Some(own) => edges::clamp(own, min, max).max(edges.pad[Y]),
                None => {
                    edges::clamp(self.heights[root.index()].offered, min, max).max(edges.pad[Y])
                }
            },
        };
        let free = line - height - edges.margin[Y];
        let cross = match align {
            crate::style::Align::Center => free / 2.0,
            crate::style::Align::End => free,
            crate::style::Align::Start | crate::style::Align::Stretch => 0.0,
        };
        (height, cross)
    }
}
