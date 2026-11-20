use rikka_image_edit::{ConvertProfile, Imaging, Intent, Profile, Recipe};
use rikka_psd::{ChannelId, Compression, ResourceData, resource_id};

use crate::develop;
use crate::error::{Error, Result};
use crate::layer;

pub(crate) fn photoshop(
    imaging: &Imaging,
    document: &rikka_psd::Document,
    from: Option<&Profile>,
    to: &Profile,
    intent: Intent,
) -> Result<rikka_psd::Document> {
    if document.color_mode != rikka_psd::ColorMode::Rgb {
        return Err(Error::ProfileNeedsRgb);
    }
    if document.depth == rikka_psd::Depth::ThirtyTwo {
        return Err(Error::ProfileOnFloat);
    }
    if to.is_gray() {
        return Err(Error::GreyProfile);
    }
    let recipe = Recipe::new().then(ConvertProfile::new(to.clone()).intent(intent));
    let pixel_layers: Vec<_> = layer::nodes(document)
        .filter_map(|(id, node)| match node {
            rikka_psd::Node::Layer(record)
                if record.kind() == rikka_psd::LayerKind::Pixel && !record.bounds.is_empty() =>
            {
                Some((id, record))
            }
            _ => None,
        })
        .collect();
    let mut converted = Vec::with_capacity(pixel_layers.len());
    for (id, record) in pixel_layers {
        let bounds = record.bounds;
        crate::document::check_memory_budget(
            bounds.width(),
            bounds.height(),
            bounds.area() * develop::DECODED_BYTES_PER_PIXEL * 2,
        )?;
        let rgba = document.layer_rgba(record)?;
        let opaque = layer::is_opaque_record(record);
        let changed = convert(imaging, &rgba, document.depth, from, &recipe)?;
        let mut record = record.clone();
        document.set_layer_rgba(
            &mut record,
            bounds.left,
            bounds.top,
            &changed,
            Compression::Rle,
        )?;
        if opaque {
            record.remove_channel(ChannelId::Transparency);
        }
        converted.push((id, record));
    }
    let composite = match document.has_real_composite() {
        true => {
            let composite = document.composite_rgba()?;
            Some(convert(imaging, &composite, document.depth, from, &recipe)?)
        }
        false => None,
    };
    let mut document = document.clone();
    if let Some(composite) = composite {
        document.set_composite_rgba(&composite, Compression::Rle)?;
    }
    for (id, record) in converted {
        if let Some(node) = layer::node_mut(&mut document, id) {
            *node.layer_mut() = record;
        }
    }
    document.resources.set(
        resource_id::ICC_PROFILE,
        ResourceData::IccProfile(to.icc().to_vec()),
    );
    document.resources.remove(resource_id::ICC_UNTAGGED);
    document.resources.remove(resource_id::THUMBNAIL);
    document.resources.remove(resource_id::THUMBNAIL_BGR);
    Ok(document)
}

fn convert(
    imaging: &Imaging,
    rgba: &rikka_psd::RgbaImage,
    depth: rikka_psd::Depth,
    from: Option<&Profile>,
    recipe: &Recipe,
) -> Result<rikka_psd::RgbaImage> {
    let picture = develop::picture_of(imaging, rgba, depth, from)?;
    let converted = picture.develop(recipe)?;
    develop::rgba_of(&converted, false)
}
