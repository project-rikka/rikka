use rikka_psd::{BlendMode, NodePath};

use crate::error::{Error, Result};
use crate::figure::{Figure, FigureId};
use crate::overlay::{Overlays, PLAIN};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerId(pub(crate) u32);

impl LayerId {
    #[must_use]
    pub fn index(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerKind {
    Pixel,
    Text,
    Shape,
    Fill,
    Adjustment,
    SmartObject,
    Group,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub kind: LayerKind,
    pub background: bool,
    pub visible: bool,
    pub opacity: f32,
    pub blend: BlendMode,
    pub clipped: bool,
    pub figures: Vec<Figure>,
    pub children: Vec<Layer>,
}

impl Layer {
    pub(crate) fn picture(visible: bool, overlays: &Overlays) -> Layer {
        Layer {
            id: LayerId(0),
            name: String::new(),
            kind: LayerKind::Pixel,
            background: true,
            visible,
            opacity: 1.0,
            blend: BlendMode::Normal,
            clipped: false,
            figures: figures_of(overlays, Some(PLAIN)),
            children: Vec::new(),
        }
    }

    pub(crate) fn from_psd_nodes(nodes: &[rikka_psd::Node], overlays: &Overlays) -> Vec<Layer> {
        let mut next = 0;
        let mut layers = gather(nodes, &mut next, overlays);
        if let Some(bottom) = layers.last_mut() {
            bottom.background = nodes.first().is_some_and(is_background);
        }
        layers
    }

    #[must_use]
    pub fn find(layers: &[Layer], id: LayerId) -> Option<&Layer> {
        layers.iter().find_map(|layer| match layer.id == id {
            true => Some(layer),
            false => Layer::find(&layer.children, id),
        })
    }

    #[must_use]
    pub fn holding(layers: &[Layer], figure: FigureId) -> Option<&Layer> {
        layers.iter().find_map(
            |layer| match layer.figures.iter().any(|other| other.id == figure) {
                true => Some(layer),
                false => Layer::holding(&layer.children, figure),
            },
        )
    }
}

fn figures_of(overlays: &Overlays, key: Option<u32>) -> Vec<Figure> {
    key.and_then(|key| overlays.get(key))
        .map(|overlay| overlay.figures.clone())
        .unwrap_or_default()
}

fn gather(nodes: &[rikka_psd::Node], next: &mut u32, overlays: &Overlays) -> Vec<Layer> {
    let mut gathered: Vec<Layer> = nodes
        .iter()
        .map(|node| {
            let id = LayerId(*next);
            *next += 1;
            let layer = node.layer();
            let (kind, children, figures) = match node {
                rikka_psd::Node::Group(group) => (
                    LayerKind::Group,
                    gather(&group.children, next, overlays),
                    Vec::new(),
                ),
                rikka_psd::Node::Layer(layer) => (
                    kind_of(layer.kind()),
                    Vec::new(),
                    figures_of(overlays, layer.id()),
                ),
            };
            Layer {
                id,
                name: layer.name().to_owned(),
                kind,
                background: false,
                visible: layer.visible(),
                opacity: layer.opacity(),
                blend: node.blend_mode(),
                clipped: layer.clipping,
                figures,
                children,
            }
        })
        .collect();
    gathered.reverse();
    gathered
}

pub(crate) fn is_background(node: &rikka_psd::Node) -> bool {
    match node {
        rikka_psd::Node::Layer(layer) => is_opaque_record(layer),
        rikka_psd::Node::Group(_) => false,
    }
}

pub(crate) fn is_opaque_record(layer: &rikka_psd::Layer) -> bool {
    layer.channel(rikka_psd::ChannelId::Transparency).is_none() && !layer.bounds.is_empty()
}

pub(crate) fn path(document: &rikka_psd::Document, id: LayerId) -> Option<NodePath> {
    document.preorder_path(usize::try_from(id.0).ok()?)
}

pub(crate) fn located(document: &rikka_psd::Document, id: LayerId) -> Result<NodePath> {
    path(document, id).ok_or(Error::UnknownLayer(id))
}

pub(crate) fn id_at(document: &rikka_psd::Document, path: &NodePath) -> Result<LayerId> {
    document
        .preorder_index(path)
        .and_then(|index| u32::try_from(index).ok())
        .map(LayerId)
        .ok_or(Error::Psd(rikka_psd::Error::NoPlace))
}

pub(crate) fn node(document: &rikka_psd::Document, id: LayerId) -> Option<&rikka_psd::Node> {
    document.node(&path(document, id)?)
}

pub(crate) fn node_mut(
    document: &mut rikka_psd::Document,
    id: LayerId,
) -> Option<&mut rikka_psd::Node> {
    let path = path(document, id)?;
    document.node_mut(&path)
}

pub(crate) fn nodes(
    document: &rikka_psd::Document,
) -> impl Iterator<Item = (LayerId, &rikka_psd::Node)> {
    (0..).map(LayerId).zip(document.preorder())
}

pub(crate) fn record_keyed_mut(
    nodes: &mut [rikka_psd::Node],
    key: u32,
) -> Option<&mut rikka_psd::Layer> {
    nodes.iter_mut().find_map(|node| match node {
        rikka_psd::Node::Group(group) => record_keyed_mut(&mut group.children, key),
        rikka_psd::Node::Layer(record) => (record.id() == Some(key)).then_some(record),
    })
}

pub(crate) fn records(node: &mut rikka_psd::Node, visit: &mut impl FnMut(&mut rikka_psd::Layer)) {
    match node {
        rikka_psd::Node::Layer(layer) => visit(layer),
        rikka_psd::Node::Group(group) => {
            visit(&mut group.layer);
            for child in &mut group.children {
                records(child, visit);
            }
        }
    }
}

fn kind_of(kind: rikka_psd::LayerKind) -> LayerKind {
    match kind {
        rikka_psd::LayerKind::Pixel => LayerKind::Pixel,
        rikka_psd::LayerKind::Text => LayerKind::Text,
        rikka_psd::LayerKind::Shape => LayerKind::Shape,
        rikka_psd::LayerKind::Fill => LayerKind::Fill,
        rikka_psd::LayerKind::Adjustment => LayerKind::Adjustment,
        rikka_psd::LayerKind::SmartObject => LayerKind::SmartObject,
        rikka_psd::LayerKind::GroupStart | rikka_psd::LayerKind::GroupEnd => LayerKind::Group,
    }
}
