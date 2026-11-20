use rikka_ui::{
    Align as RikkaAlign, Extent as Dimensions, Inset, Justify as RikkaJustify, Layout, LayoutNode,
    LayoutTree, Length, MeasureSpec, Size,
};

use crate::engine::Engine;
use crate::spec::{Align, BoxStyle, Direction, Extent, Justify, Placed, Spec};

#[derive(Debug)]
pub struct Rikka {
    tree: LayoutTree,
    nodes: Vec<LayoutNode>,
    root: BoxStyle,
    viewport: Size,
}

fn length(value: Option<f32>) -> Length {
    value.map_or(Length::HUG, Length::px)
}

fn layout(style: &BoxStyle) -> Layout {
    let mut layout = match style.direction {
        Direction::Row => Layout::row(),
        Direction::Column => Layout::column(),
    };
    layout.wrap = style.wrap;
    layout.gap = Size::new(style.gap, style.gap);
    layout.padding = Inset::exact(style.padding, style.padding, style.padding, style.padding);
    layout.margin = Inset::exact(style.margin, style.margin, style.margin, style.margin);
    layout.size = Dimensions::new(length(style.width), length(style.height));
    layout.basis = length(style.basis);
    layout.grow = style.grow;
    layout.shrink = style.shrink;
    layout.align_items = Some(match style.align {
        Align::Start => RikkaAlign::Start,
        Align::Center => RikkaAlign::Center,
        Align::End => RikkaAlign::End,
        Align::Stretch => RikkaAlign::Stretch,
    });
    layout.justify = Some(match style.justify {
        Justify::Start => RikkaJustify::Start,
        Justify::Center => RikkaJustify::Center,
        Justify::End => RikkaJustify::End,
    });
    layout
}

fn measure(content: Extent) -> MeasureSpec {
    MeasureSpec::Fixed(Size::new(content.width, content.height))
}

impl Rikka {
    fn add(&mut self, spec: &Spec) -> LayoutNode {
        let node = match spec.content {
            Some(content) => self.tree.leaf(layout(&spec.style), measure(content)),
            None => self.tree.container(layout(&spec.style)),
        };
        self.nodes.push(node);
        let children: Vec<LayoutNode> = spec.children.iter().map(|child| self.add(child)).collect();
        if !children.is_empty() {
            self.tree.set_children(node, &children);
        }
        node
    }

    fn place(&self, node: LayoutNode, parent: (f32, f32), placed: &mut Vec<Placed>) {
        let rect = self.tree.rect(node);
        let origin = (parent.0 + rect.origin.x, parent.1 + rect.origin.y);
        placed.push(Placed {
            x: origin.0,
            y: origin.1,
            width: rect.width(),
            height: rect.height(),
        });
        for &child in self.tree.children(node) {
            self.place(child, origin, placed);
        }
    }
}

impl Engine for Rikka {
    const NAME: &'static str = "rikka";

    fn build(spec: &Spec) -> Self {
        let root = spec.root_extent();
        let mut engine = Self {
            tree: LayoutTree::new(),
            nodes: Vec::with_capacity(spec.count()),
            root: spec.style,
            viewport: Size::new(root.width, root.height),
        };
        engine.add(spec);
        engine
    }

    fn compute(&mut self) {
        self.tree.solve(self.nodes[0], self.viewport);
    }

    fn resize(&mut self, width: f32) {
        self.root.width = Some(width);
        self.viewport.width = width;
        self.tree.set_layout(self.nodes[0], layout(&self.root));
    }

    fn resize_leaf(&mut self, node: usize, content: Extent) {
        self.tree
            .set_measure(self.nodes[node], Some(measure(content)));
    }

    fn placed(&self) -> Vec<Placed> {
        let mut placed = Vec::with_capacity(self.nodes.len());
        self.place(self.nodes[0], (0.0, 0.0), &mut placed);
        placed
    }
}
