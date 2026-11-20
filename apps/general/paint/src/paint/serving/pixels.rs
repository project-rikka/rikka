use paint_assistant::Rect;
use paint_document::{Document, LayerId, Region};
use rikka_image::ColorType;
use rikka_image_edit::{ConvertProfile, Crop, Picture, Profile, Recipe, Resampling, Resize};

pub(super) fn source(document: &Document, layer: Option<LayerId>) -> Result<Picture, String> {
    match layer {
        Some(layer) => document.layer_picture(layer),
        None => document.composite(),
    }
    .map_err(|error| error.to_string())
}

pub(super) fn rgba(
    picture: &Picture,
    region: Rect,
    size: (u32, u32),
    resampling: Resampling,
) -> Result<Vec<u8>, String> {
    let mut recipe = Recipe::new();
    if (region.left, region.top, region.width, region.height)
        != (0, 0, picture.width(), picture.height())
    {
        recipe.push(Crop::new(
            region.left,
            region.top,
            region.width,
            region.height,
        ));
    }
    if size != (region.width, region.height) {
        recipe.push(Resize::new(size.0, size.1).resampling(resampling));
    }
    if let Some(profile) = picture.profile()
        && *profile != Profile::srgb()
    {
        recipe.push(ConvertProfile::new(Profile::srgb()));
    }
    let image = picture
        .develop(&recipe)
        .and_then(|frame| frame.to_image())
        .map_err(|error| format!("the picture could not be read: {error}"))?;
    Ok(image.convert(ColorType::Rgba8).as_bytes().to_vec())
}

pub(super) fn contains(region: &Region, x: u32, y: u32) -> bool {
    region
        .row(y)
        .iter()
        .any(|span| span.start <= x && x < span.end)
}

pub(super) fn clipped(requested: Rect, width: u32, height: u32) -> Result<Rect, String> {
    let right = (requested.left + requested.width).min(width);
    let bottom = (requested.top + requested.height).min(height);
    if requested.left >= right || requested.top >= bottom {
        return Err(format!(
            "x {}–{}, y {}–{} lies outside the {width}×{height} picture",
            requested.left,
            requested.left + requested.width,
            requested.top,
            requested.top + requested.height
        ));
    }
    Ok(Rect {
        left: requested.left,
        top: requested.top,
        width: right - requested.left,
        height: bottom - requested.top,
    })
}

pub(super) fn whole(width: u32, height: u32) -> Rect {
    Rect {
        left: 0,
        top: 0,
        width,
        height,
    }
}
