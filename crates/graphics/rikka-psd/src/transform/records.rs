use crate::compression::{Compression, Encoded};
use crate::document::Document;
use crate::error::Result;
use crate::geometry::Rect;
use crate::layers::{Channel, ChannelId, Layer, Node};
use crate::pixels::{Depth, Plane};

pub(crate) fn each(nodes: &[Node], visit: &mut impl FnMut(&Layer) -> Result<()>) -> Result<()> {
    for node in nodes {
        match node {
            Node::Layer(layer) => visit(layer)?,
            Node::Group(group) => {
                visit(&group.layer)?;
                each(&group.children, visit)?;
                visit(&group.end)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn each_mut(nodes: &mut [Node], visit: &mut impl FnMut(&mut Layer)) {
    for node in nodes {
        match node {
            Node::Layer(layer) => visit(layer),
            Node::Group(group) => {
                visit(&mut group.layer);
                each_mut(&mut group.children, visit);
                visit(&mut group.end);
            }
        }
    }
}

pub(crate) fn has_background(document: &Document) -> bool {
    matches!(
        document.layers.first(),
        Some(Node::Layer(layer))
            if layer.channel(ChannelId::Transparency).is_none()
                && layer.section().is_none()
                && !layer.bounds.is_empty()
    )
}

pub(crate) struct LayerGeometry {
    pub(crate) bounds: Rect,
    pub(crate) mask: Option<(Rect, Option<Rect>)>,
    pub(crate) channels: Option<Vec<Channel>>,
}

pub(crate) trait Remap {
    fn remap_composite(&mut self, index: usize, plane: Plane) -> Result<Plane>;

    fn remap_layer(&mut self, layer: &Layer, background: bool) -> Result<LayerGeometry>;
}

pub(crate) struct TransformPlan {
    composite: Vec<Encoded>,
    records: Vec<LayerGeometry>,
}

impl TransformPlan {
    pub(crate) fn plan(document: &Document, remap: &mut impl Remap) -> Result<Self> {
        let composite = document
            .composite
            .planes
            .iter()
            .enumerate()
            .map(|(index, encoded)| {
                let plane = encoded.decode(document.width, document.height, document.depth)?;
                Ok(Encoded::encode(
                    &remap.remap_composite(index, plane)?,
                    composite_compression(encoded),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut background = has_background(document);
        let mut records = Vec::new();
        each(&document.layers, &mut |layer| {
            records.push(remap.remap_layer(layer, background)?);
            background = false;
            Ok(())
        })?;
        Ok(Self { composite, records })
    }

    pub(crate) fn commit(self, document: &mut Document) {
        document.composite.planes = self.composite;
        let mut records = self.records.into_iter();
        each_mut(&mut document.layers, &mut |layer| {
            let geometry = records.next().expect("one record per layer");
            layer.bounds = geometry.bounds;
            if let (Some(mask), Some((bounds, real))) = (&mut layer.mask, geometry.mask) {
                mask.bounds = bounds;
                if let (Some(slot), Some(real)) = (&mut mask.real, real) {
                    slot.bounds = real;
                }
            }
            if let Some(channels) = geometry.channels {
                layer.channels = channels;
            }
        });
    }
}

fn composite_compression(encoded: &Encoded) -> Compression {
    match encoded.compression() {
        Compression::Raw => Compression::Raw,
        _ => Compression::Rle,
    }
}

pub(crate) fn mapped_mask(
    layer: &Layer,
    map: impl Fn(Rect) -> Result<Rect>,
) -> Result<Option<(Rect, Option<Rect>)>> {
    layer
        .mask
        .map(|mask| {
            Ok((
                map(mask.bounds)?,
                mask.real.map(|real| map(real.bounds)).transpose()?,
            ))
        })
        .transpose()
}

pub(crate) fn rebuild_channels(
    layer: &Layer,
    depth: Depth,
    mut remap: impl FnMut(&Channel, Plane) -> Result<Plane>,
) -> Result<Vec<Channel>> {
    layer
        .channels
        .iter()
        .map(|channel| {
            let Some(plane) = layer.decode_channel(channel.id, depth)? else {
                return Ok(channel.clone());
            };
            let plane = remap(channel, plane)?;
            Ok(Channel::new(channel.id, &plane, channel.data.compression()))
        })
        .collect()
}
