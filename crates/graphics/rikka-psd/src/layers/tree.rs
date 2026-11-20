use crate::blend::BlendMode;
use crate::geometry::Rect;
use crate::info::{Info, Section, SectionKind};
use crate::layers::layer::{Layer, LayerFlags};
use crate::layers::path::NodePath;

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub layer: Layer,
    pub children: Vec<Node>,
    pub end: Box<Layer>,
}

impl Group {
    pub fn new(name: impl Into<String>) -> Self {
        let mut layer = Layer::new(name);
        layer.flags = LayerFlags(0x18);
        layer.set_info(Info::Section(Section {
            kind: SectionKind::OpenFolder,
            blend_mode: Some(BlendMode::PassThrough),
            sub_kind: None,
            nested: false,
        }));
        let mut end = Layer::new("</Layer group>");
        end.flags = LayerFlags(0x18);
        end.set_info(Info::Section(Section::new(SectionKind::BoundingDivider)));
        Self {
            layer,
            children: Vec::new(),
            end: Box::new(end),
        }
    }

    #[must_use]
    pub fn with_children(mut self, children: Vec<Node>) -> Self {
        self.children = children;
        self
    }

    pub fn name(&self) -> &str {
        self.layer.name()
    }

    pub fn blend_mode(&self) -> BlendMode {
        self.layer
            .section()
            .and_then(|section| section.blend_mode)
            .unwrap_or(self.layer.blend_mode)
    }

    pub fn set_blend_mode(&mut self, mode: BlendMode) {
        let mut section = self
            .layer
            .section()
            .copied()
            .unwrap_or(Section::new(SectionKind::OpenFolder));
        section.blend_mode = Some(mode);
        self.layer.set_info(Info::Section(section));
        self.layer.blend_mode = if mode == BlendMode::PassThrough {
            BlendMode::Normal
        } else {
            mode
        };
    }

    pub fn is_open(&self) -> bool {
        self.layer.section_kind() != SectionKind::ClosedFolder
    }

    pub fn set_open(&mut self, open: bool) {
        let mut section = self
            .layer
            .section()
            .copied()
            .unwrap_or(Section::new(SectionKind::OpenFolder));
        section.kind = if open {
            SectionKind::OpenFolder
        } else {
            SectionKind::ClosedFolder
        };
        self.layer.set_info(Info::Section(section));
    }

    pub fn bounds(&self) -> Rect {
        self.children
            .iter()
            .fold(Rect::EMPTY, |bounds, child| bounds.union(&child.bounds()))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Layer(Layer),
    Group(Group),
}

impl Node {
    pub fn layer(&self) -> &Layer {
        match self {
            Node::Layer(layer) => layer,
            Node::Group(group) => &group.layer,
        }
    }

    pub fn layer_mut(&mut self) -> &mut Layer {
        match self {
            Node::Layer(layer) => layer,
            Node::Group(group) => &mut group.layer,
        }
    }

    pub fn name(&self) -> &str {
        self.layer().name()
    }

    pub fn bounds(&self) -> Rect {
        match self {
            Node::Layer(layer) => layer.bounds,
            Node::Group(group) => group.bounds(),
        }
    }

    pub fn as_group(&self) -> Option<&Group> {
        match self {
            Node::Group(group) => Some(group),
            Node::Layer(_) => None,
        }
    }

    pub fn as_group_mut(&mut self) -> Option<&mut Group> {
        match self {
            Node::Group(group) => Some(group),
            Node::Layer(_) => None,
        }
    }

    pub fn blend_mode(&self) -> BlendMode {
        match self {
            Node::Layer(layer) => layer.blend_mode,
            Node::Group(group) => group.blend_mode(),
        }
    }
}

impl From<Layer> for Node {
    fn from(layer: Layer) -> Self {
        Node::Layer(layer)
    }
}

impl From<Group> for Node {
    fn from(group: Group) -> Self {
        Node::Group(group)
    }
}

pub(crate) fn build(records: Vec<Layer>) -> Vec<Node> {
    let mut stack: Vec<(Vec<Node>, Option<Layer>)> = vec![(Vec::new(), None)];
    for record in records {
        match record.section_kind() {
            SectionKind::BoundingDivider => stack.push((Vec::new(), Some(record))),
            SectionKind::OpenFolder | SectionKind::ClosedFolder if stack.len() > 1 => {
                let (children, end) = stack.pop().expect("stack holds a frame");
                let group = Group {
                    layer: record,
                    children,
                    end: Box::new(end.expect("inner frame has a divider")),
                };
                stack
                    .last_mut()
                    .expect("root frame remains")
                    .0
                    .push(Node::Group(group));
            }
            _ => stack
                .last_mut()
                .expect("root frame remains")
                .0
                .push(Node::Layer(record)),
        }
    }
    while stack.len() > 1 {
        let (children, end) = stack.pop().expect("stack holds a frame");
        let parent = &mut stack.last_mut().expect("root frame remains").0;
        if let Some(end) = end {
            parent.push(Node::Layer(end));
        }
        parent.extend(children);
    }
    stack.pop().map(|(nodes, _)| nodes).unwrap_or_default()
}

pub(crate) fn flatten<'a>(nodes: &'a [Node], out: &mut Vec<&'a Layer>) {
    for node in nodes {
        match node {
            Node::Layer(layer) => out.push(layer),
            Node::Group(group) => {
                out.push(&group.end);
                flatten(&group.children, out);
                out.push(&group.layer);
            }
        }
    }
}

pub(crate) struct Preorder<'a> {
    levels: Vec<std::slice::Iter<'a, Node>>,
}

impl<'a> Preorder<'a> {
    pub(crate) fn new(nodes: &'a [Node]) -> Self {
        Self {
            levels: vec![nodes.iter()],
        }
    }
}

impl<'a> Iterator for Preorder<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<&'a Node> {
        loop {
            let level = self.levels.last_mut()?;
            match level.next() {
                Some(node) => {
                    if let Node::Group(group) = node {
                        self.levels.push(group.children.iter());
                    }
                    return Some(node);
                }
                None => {
                    self.levels.pop();
                }
            }
        }
    }
}

pub(crate) fn preorder_path(nodes: &[Node], position: usize) -> Option<NodePath> {
    let mut parent = Vec::new();
    let mut level = nodes;
    let mut left = position;
    'descend: loop {
        for (index, node) in level.iter().enumerate() {
            if left == 0 {
                return Some(NodePath::from_parts(parent, index));
            }
            let size = size(node);
            match node {
                Node::Group(group) if left < size => {
                    parent.push(index);
                    level = &group.children;
                    left -= 1;
                    continue 'descend;
                }
                _ => left -= size,
            }
        }
        return None;
    }
}

pub(crate) fn preorder_index(nodes: &[Node], path: &NodePath) -> Option<usize> {
    let mut position = 0;
    let level = path.parent().iter().try_fold(nodes, |level, &index| {
        let Node::Group(group) = level.get(index)? else {
            return None;
        };
        position += level[..index].iter().map(size).sum::<usize>() + 1;
        Some(group.children.as_slice())
    })?;
    level.get(path.index())?;
    Some(position + level[..path.index()].iter().map(size).sum::<usize>())
}

pub(crate) fn siblings<'a>(nodes: &'a [Node], parent: &[usize]) -> Option<&'a [Node]> {
    parent
        .iter()
        .try_fold(nodes, |level, &index| match level.get(index)? {
            Node::Group(group) => Some(group.children.as_slice()),
            Node::Layer(_) => None,
        })
}

pub(crate) fn siblings_mut<'a>(
    nodes: &'a mut Vec<Node>,
    parent: &[usize],
) -> Option<&'a mut Vec<Node>> {
    parent
        .iter()
        .try_fold(nodes, |level, &index| match level.get_mut(index)? {
            Node::Group(group) => Some(&mut group.children),
            Node::Layer(_) => None,
        })
}

fn size(node: &Node) -> usize {
    1 + match node {
        Node::Layer(_) => 0,
        Node::Group(group) => group.children.iter().map(size).sum(),
    }
}
