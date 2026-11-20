use rikka_psd::{Channel, ChannelId, Compression, Plane};

use crate::area::Area;
use crate::error::Result;
use crate::layer::{self, LayerId};
use crate::region::{Region, Span};

const CLEARING_BYTES_PER_PIXEL: u64 = 8;

pub(crate) fn photoshop(
    document: &mut rikka_psd::Document,
    area: Area,
    shaped: Option<&Region>,
) -> Result<()> {
    let cleared = match shaped {
        Some(region) => cleared_outside(document, region)?,
        None => Vec::new(),
    };
    let rect =
        rikka_psd::Rect::from_size(area.left as i32, area.top as i32, area.width, area.height);
    document.crop(rect)?;
    for (id, channel) in cleared {
        if let Some(node) = layer::node_mut(document, id) {
            node.layer_mut().put_channel(channel);
        }
    }
    Ok(())
}

fn cleared_outside(
    document: &rikka_psd::Document,
    region: &Region,
) -> Result<Vec<(LayerId, Channel)>> {
    let pixel_layers = layer::nodes(document).filter_map(|(id, node)| match node {
        rikka_psd::Node::Layer(layer)
            if layer.kind() == rikka_psd::LayerKind::Pixel && !layer.bounds.is_empty() =>
        {
            Some((id, layer))
        }
        _ => None,
    });
    let mut cleared = Vec::new();
    for (id, layer) in pixel_layers {
        if let Some(channel) = cleared_alpha(document, layer, region)? {
            cleared.push((id, channel));
        }
    }
    Ok(cleared)
}

fn cleared_alpha(
    document: &rikka_psd::Document,
    layer: &rikka_psd::Layer,
    region: &Region,
) -> Result<Option<Channel>> {
    let bounds = layer.bounds;
    let (width, height) = (bounds.width(), bounds.height());
    if let (Ok(left), Ok(top)) = (u32::try_from(bounds.left), u32::try_from(bounds.top))
        && region.encloses(Area::new(left, top, width, height))
    {
        return Ok(None);
    }
    crate::document::check_memory_budget(width, height, bounds.area() * CLEARING_BYTES_PER_PIXEL)?;
    let alpha = layer.decode_channel(ChannelId::Transparency, document.depth)?;
    let mut changed = alpha.is_none();
    let compression = layer
        .channel(ChannelId::Transparency)
        .map_or(Compression::Rle, |channel| channel.data.compression());
    let mut values = alpha.map_or_else(
        || vec![1.0; width as usize * height as usize],
        |plane| plane.to_normalized(),
    );
    let mut inside = Vec::new();
    for (row, line) in values.chunks_mut(width as usize).enumerate() {
        let y = bounds.top + row as i32;
        inside.clear();
        if let Ok(y) = u32::try_from(y) {
            inside.extend(
                region
                    .row(y)
                    .iter()
                    .filter_map(|span| clip_span(*span, bounds)),
            );
        }
        let mut from = 0;
        for span in inside.iter().copied().chain([Span::full(width, width)]) {
            for value in &mut line[from as usize..span.start as usize] {
                changed |= *value != 0.0;
                *value = 0.0;
            }
            if span.coverage < Span::FULL {
                let coverage = f32::from(span.coverage) / f32::from(Span::FULL);
                for value in &mut line[span.start as usize..span.end as usize] {
                    changed |= *value != 0.0;
                    *value *= coverage;
                }
            }
            from = span.end;
        }
    }
    if !changed {
        return Ok(None);
    }
    let plane = Plane::from_normalized(width, height, document.depth, &values)?;
    Ok(Some(Channel::new(
        ChannelId::Transparency,
        &plane,
        compression,
    )))
}

fn clip_span(span: Span, bounds: rikka_psd::Rect) -> Option<Span> {
    let start = i64::from(span.start).max(i64::from(bounds.left));
    let end = i64::from(span.end).min(i64::from(bounds.right));
    (start < end).then(|| Span {
        start: (start - i64::from(bounds.left)) as u32,
        end: (end - i64::from(bounds.left)) as u32,
        coverage: span.coverage,
    })
}
