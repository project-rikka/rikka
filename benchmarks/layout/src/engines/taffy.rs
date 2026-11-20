use taffy::prelude::{
    AlignContent, AlignItems, AvailableSpace, Dimension, Display, FlexDirection, FlexWrap,
    JustifyContent, LengthPercentage, LengthPercentageAuto, NodeId, Rect, Size, Style, TaffyTree,
};

use crate::engine::Engine;
use crate::spec::{Align, BoxStyle, Direction, Extent, Justify, Placed, Spec};

pub struct Taffy {
    tree: TaffyTree<Extent>,
    nodes: Vec<NodeId>,
    root: BoxStyle,
    viewport: Size<AvailableSpace>,
}

fn dimension(value: Option<f32>) -> Dimension {
    value.map_or(Dimension::auto(), Dimension::length)
}

fn style(style: &BoxStyle, container: bool) -> Style {
    let padding = LengthPercentage::length(style.padding);
    let margin = LengthPercentageAuto::length(style.margin);
    let gap = LengthPercentage::length(style.gap);
    let floor = match container {
        true => LengthPercentageAuto::length(0.0),
        false => LengthPercentageAuto::auto(),
    };
    Style {
        display: Display::Flex,
        flex_direction: match style.direction {
            Direction::Row => FlexDirection::Row,
            Direction::Column => FlexDirection::Column,
        },
        flex_wrap: match style.wrap {
            true => FlexWrap::Wrap,
            false => FlexWrap::NoWrap,
        },
        size: Size {
            width: dimension(style.width),
            height: dimension(style.height),
        },
        min_size: Size {
            width: floor,
            height: floor,
        },
        flex_grow: style.grow,
        flex_shrink: style.shrink,
        flex_basis: dimension(style.basis),
        padding: Rect {
            left: padding,
            right: padding,
            top: padding,
            bottom: padding,
        },
        margin: Rect {
            left: margin,
            right: margin,
            top: margin,
            bottom: margin,
        },
        gap: Size {
            width: gap,
            height: gap,
        },
        align_items: Some(match style.align {
            Align::Start => AlignItems::START,
            Align::Center => AlignItems::CENTER,
            Align::End => AlignItems::END,
            Align::Stretch => AlignItems::STRETCH,
        }),
        justify_content: Some(match style.justify {
            Justify::Start => JustifyContent::START,
            Justify::Center => JustifyContent::CENTER,
            Justify::End => JustifyContent::END,
        }),
        align_content: Some(AlignContent::STRETCH),
        ..Style::default()
    }
}

impl Taffy {
    fn add(&mut self, spec: &Spec) -> NodeId {
        let container = spec.content.is_none();
        let node = match spec.content {
            Some(content) => self
                .tree
                .new_leaf_with_context(style(&spec.style, false), content),
            None => self.tree.new_leaf(style(&spec.style, container)),
        }
        .expect("taffy should accept the node");
        self.nodes.push(node);
        let children: Vec<NodeId> = spec.children.iter().map(|child| self.add(child)).collect();
        if !children.is_empty() {
            self.tree
                .set_children(node, &children)
                .expect("taffy should accept the children");
        }
        node
    }

    fn place(&self, node: NodeId, parent: (f32, f32), placed: &mut Vec<Placed>) {
        let layout = self.tree.layout(node).expect("node should exist");
        let origin = (parent.0 + layout.location.x, parent.1 + layout.location.y);
        placed.push(Placed {
            x: origin.0,
            y: origin.1,
            width: layout.size.width,
            height: layout.size.height,
        });
        for &child in self.tree.children(node).expect("node should exist").iter() {
            self.place(child, origin, placed);
        }
    }
}

impl Engine for Taffy {
    const NAME: &'static str = "taffy";

    fn build(spec: &Spec) -> Self {
        let root = spec.root_extent();
        let mut engine = Self {
            tree: TaffyTree::with_capacity(spec.count()),
            nodes: Vec::with_capacity(spec.count()),
            root: spec.style,
            viewport: Size {
                width: AvailableSpace::Definite(root.width),
                height: AvailableSpace::Definite(root.height),
            },
        };
        engine.add(spec);
        engine
    }

    fn compute(&mut self) {
        self.tree
            .compute_layout_with_measure(
                self.nodes[0],
                self.viewport,
                |inputs, _node, content, style| {
                    taffy::compute_leaf_layout(
                        inputs,
                        style,
                        |_, _| 0.0,
                        |known, _available| match content {
                            Some(content) => Size {
                                width: known.width.unwrap_or(content.width),
                                height: known.height.unwrap_or(content.height),
                            },
                            None => Size::ZERO,
                        },
                    )
                },
            )
            .expect("taffy should lay out the tree");
    }

    fn resize(&mut self, width: f32) {
        self.root.width = Some(width);
        self.viewport.width = AvailableSpace::Definite(width);
        self.tree
            .set_style(self.nodes[0], style(&self.root, true))
            .expect("root should exist");
    }

    fn resize_leaf(&mut self, node: usize, content: Extent) {
        let id = self.nodes[node];
        if let Some(stored) = self.tree.get_node_context_mut(id) {
            *stored = content;
        }
        self.tree.mark_dirty(id).expect("leaf should exist");
    }

    fn placed(&self) -> Vec<Placed> {
        let mut placed = Vec::with_capacity(self.nodes.len());
        self.place(self.nodes[0], (0.0, 0.0), &mut placed);
        placed
    }
}

impl std::fmt::Debug for Taffy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Taffy")
            .field("nodes", &self.nodes.len())
            .finish()
    }
}
