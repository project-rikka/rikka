use rikka_types::Rect;
use rikka_vector::{Area, Canvas, FillRule, Mask, Path, Transform};

use crate::render::bounds::{unit_box, within};
use crate::render::{Host, draw_children, draw_clipped, fill_rect};
use crate::tree::{Clip, Masking, Node, Units};

fn placement(units: Units, object: Option<Rect>) -> Option<Transform> {
    match units {
        Units::UserSpace => Some(Transform::IDENTITY),
        Units::BoundingBox => object.map(unit_box),
    }
}

pub(crate) fn simple_clip<'c>(
    clip: &'c Clip,
    object: Option<Rect>,
    transform: &Transform,
) -> Option<(&'c Path, Transform, FillRule)> {
    if clip.clip.is_some() {
        return None;
    }
    let [Node::Shape(shape)] = clip.content.children.as_slice() else {
        return None;
    };
    let rule = shape.fill.as_ref()?.rule;
    let unit = placement(clip.units, object)?;
    let placed = shape
        .transform
        .then(unit)
        .then(clip.transform)
        .then(*transform);
    Some((&shape.path, placed, rule))
}

fn empty(area: Area) -> Mask {
    let mut mask = Mask::default();
    if let Ok(pixmap) = rikka_vector::Pixmap::new(area.width, area.height) {
        mask = Mask::from_alpha(&pixmap);
    }
    mask
}

fn local(transform: &Transform, area: Area) -> Transform {
    transform.then(Transform::translate(
        -(area.left as f32),
        -(area.top as f32),
    ))
}

fn multiply(mask: &mut Mask, other: &Mask) {
    for (value, other) in mask.data_mut().iter_mut().zip(other.data()) {
        *value = ((u32::from(*value) * u32::from(*other) + 127) / 255) as u8;
    }
}

pub(crate) fn clip_stencil(
    clip: &Clip,
    object: Option<Rect>,
    transform: &Transform,
    host: Host,
    area: Area,
) -> Mask {
    let Some(unit) = placement(clip.units, object) else {
        return empty(area);
    };
    let mut canvas = Canvas::new();
    canvas.begin(area.width, area.height);
    let placed = unit.then(clip.transform).then(local(transform, area));
    draw_children(&clip.content, &placed, host, &mut canvas);
    let Ok(pixmap) = canvas.to_pixmap() else {
        return empty(area);
    };
    let mut mask = Mask::from_alpha(&pixmap);
    if let Some(nested) = clip.clip.as_deref() {
        multiply(
            &mut mask,
            &clip_stencil(nested, object, transform, host, area),
        );
    }
    mask
}

pub(crate) fn mask_stencil(
    masking: &Masking,
    object: Option<Rect>,
    transform: &Transform,
    host: Host,
    area: Area,
) -> Option<Mask> {
    let rect = within(masking.rect, masking.units, object)?;
    let content = placement(masking.content_units, object)?;
    let mut canvas = Canvas::new();
    canvas.begin(area.width, area.height);
    let placed = local(transform, area);
    let (window, rule) = fill_rect(rect);
    draw_clipped(&mut canvas, &window, &placed, rule, |canvas| {
        draw_children(&masking.content, &content.then(placed), host, canvas);
    });
    let pixmap = canvas.to_pixmap().ok()?;
    let mut mask = match (masking.luminance, masking.linear) {
        (false, _) => Mask::from_alpha(&pixmap),
        (true, false) => Mask::from_luminance(&pixmap),
        (true, true) => Mask::from_linear_luminance(&pixmap),
    };
    if let Some(nested) = masking.mask.as_deref() {
        multiply(
            &mut mask,
            &mask_stencil(nested, object, transform, host, area)?,
        );
    }
    Some(mask)
}
