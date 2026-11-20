use rikka_psd::{BlendMode, ChannelId, Composite, Compression, Info, Node, NodePath, RgbaImage};

use crate::arrange;
use crate::error::{Error, Result};
use crate::layer::{self, LayerId};
use crate::save::RENDERED_BYTES_PER_PIXEL;

const BACKGROUND: &str = "Background";

pub(crate) fn down(document: &mut rikka_psd::Document, id: LayerId) -> Result<LayerId> {
    let upper_at = layer::located(document, id)?;
    let lower_at = upper_at.sibling(upper_at.index().checked_sub(1).ok_or_else(nothing_below)?);
    let Some(Node::Layer(lower)) = document.node(&lower_at) else {
        return Err(nothing_below());
    };
    let upper = document
        .node(&upper_at)
        .ok_or(Error::UnknownLayer(id))?
        .clone();
    let mut base = lower.clone();
    base.opacity = u8::MAX;
    base.blend_mode = BlendMode::Normal;
    base.clipping = false;
    base.set_visible(true);
    let pixels = rendered(&alone(document, vec![Node::Layer(base), upper]))?;
    let mut merged = rikka_psd::Layer::new(lower.name());
    merged.opacity = lower.opacity;
    merged.blend_mode = lower.blend_mode;
    merged.clipping = lower.clipping;
    merged.set_visible(lower.visible());
    if let Some(id) = lower.id() {
        merged.set_info(Info::Id(id));
    }
    let opaque = layer::is_opaque_record(lower);
    painted(document, &mut merged, &pixels, opaque)?;
    document
        .remove_node(&upper_at)
        .ok_or(Error::UnknownLayer(id))?;
    *document.node_mut(&lower_at).ok_or_else(nothing_below)? = Node::Layer(merged);
    layer::id_at(document, &lower_at)
}

pub(crate) fn visible(document: &mut rikka_psd::Document) -> Result<LayerId> {
    let Some(bottom) = document
        .layers
        .iter()
        .position(|node| node.layer().visible())
    else {
        return Err(Error::NothingVisible);
    };
    let opaque = bottom == 0 && layer::is_background(&document.layers[0]);
    let name = first_shown(&document.layers)
        .map(|record| record.name().to_owned())
        .unwrap_or_default();
    let pixels = rendered(document)?;
    let mut merged = rikka_psd::Layer::new(name);
    arrange::identify(document, &mut merged);
    painted(document, &mut merged, &pixels, opaque)?;
    strip_shown(&mut document.layers);
    let at = NodePath::root(bottom.min(document.layers.len()));
    document.insert_node(&at, Node::Layer(merged))?;
    layer::id_at(document, &at)
}

pub(crate) fn flatten(document: &mut rikka_psd::Document) -> Result<LayerId> {
    let composite = rendered(document)?;
    let pixels = RgbaImage {
        pixels: composite
            .pixels
            .iter()
            .map(|&[r, g, b, a]| {
                let alpha = a.clamp(0.0, 1.0);
                let [r, g, b] = [r, g, b].map(|channel| channel * alpha + 1.0 - alpha);
                [r, g, b, 1.0]
            })
            .collect(),
        ..composite
    };
    let mut background = rikka_psd::Layer::new(BACKGROUND);
    arrange::identify(document, &mut background);
    painted(document, &mut background, &pixels, true)?;
    document.layers = vec![Node::Layer(background)];
    Ok(LayerId(0))
}

fn painted(
    document: &rikka_psd::Document,
    record: &mut rikka_psd::Layer,
    pixels: &RgbaImage,
    opaque: bool,
) -> Result<()> {
    document.set_layer_rgba(record, 0, 0, pixels, Compression::Rle)?;
    if opaque {
        record.remove_channel(ChannelId::Transparency);
    }
    Ok(())
}

fn rendered(document: &rikka_psd::Document) -> Result<RgbaImage> {
    let (width, height) = (document.width, document.height);
    crate::document::check_memory_budget(
        width,
        height,
        u64::from(width) * u64::from(height) * RENDERED_BYTES_PER_PIXEL,
    )?;
    rikka_psd_render::render_rgba(document).map_err(Error::from)
}

fn alone(document: &rikka_psd::Document, layers: Vec<Node>) -> rikka_psd::Document {
    rikka_psd::Document {
        version: document.version,
        width: document.width,
        height: document.height,
        depth: document.depth,
        color_mode: document.color_mode,
        color_mode_data: document.color_mode_data.clone(),
        resources: document.resources.clone(),
        layers,
        merged_alpha: document.merged_alpha,
        global_mask: document.global_mask,
        info: document.info.clone(),
        composite: Composite::default(),
    }
}

fn first_shown(nodes: &[Node]) -> Option<&rikka_psd::Layer> {
    nodes
        .iter()
        .filter(|node| node.layer().visible())
        .find_map(|node| match node {
            Node::Layer(record) => Some(record),
            Node::Group(group) => first_shown(&group.children),
        })
}

fn strip_shown(nodes: &mut Vec<Node>) {
    nodes.retain_mut(|node| {
        if !node.layer().visible() {
            return true;
        }
        match node {
            Node::Layer(_) => false,
            Node::Group(group) => {
                strip_shown(&mut group.children);
                !group.children.is_empty()
            }
        }
    });
}

fn nothing_below() -> Error {
    Error::NothingBelow
}
