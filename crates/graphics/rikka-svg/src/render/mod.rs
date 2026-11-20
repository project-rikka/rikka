pub(crate) mod bounds;
mod filters;
mod glyphs;
mod inks;
mod lighting;
mod noise;
mod pictures;
mod stencils;

use rikka_types::Rect;
use rikka_vector::{
    Antialiasing, Area, Canvas, Composite, Contours, FILL_FLATNESS, FillRule, Path, Stencil,
    Stroke, Stroker, Transform,
};

use crate::render::bounds::{content_bounds, node_bounds, scales};
use crate::render::inks::with_paint;
use crate::render::stencils::{clip_stencil, mask_stencil, simple_clip};
use crate::tree::{Group, Node, Outline, Shape, StrokeSpace};

const FARTHEST: f32 = 1.0e7;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Host {
    scale: f32,
}

impl Host {
    pub(crate) fn of(transform: &Transform) -> Host {
        Host {
            scale: scales(transform).x,
        }
    }
}

pub(crate) fn covering(bounds: Rect) -> Area {
    let low = |value: f32| value.clamp(-FARTHEST, FARTHEST).floor() as i32;
    let high = |value: f32| value.clamp(-FARTHEST, FARTHEST).ceil() as i32;
    let (left, top) = (low(bounds.min_x()) - 1, low(bounds.min_y()) - 1);
    Area {
        left,
        top,
        width: (high(bounds.max_x()) + 1 - left).max(0) as u32,
        height: (high(bounds.max_y()) + 1 - top).max(0) as u32,
    }
}

pub(crate) fn draw_node(node: &Node, transform: &Transform, host: Host, canvas: &mut Canvas) {
    match node {
        Node::Group(group) => draw_group(group, transform, host, canvas),
        Node::Shape(shape) => draw_shape(shape, transform, host, canvas),
        Node::Picture(picture) => pictures::draw(picture, transform, host, canvas),
        Node::Glyph(glyph) => glyphs::draw(glyph, transform, host, canvas),
    }
}

pub(crate) fn draw_children(group: &Group, transform: &Transform, host: Host, canvas: &mut Canvas) {
    for child in &group.children {
        draw_node(child, transform, host, canvas);
    }
}

pub(crate) fn draw_group(group: &Group, parent: &Transform, host: Host, canvas: &mut Canvas) {
    let transform = group.transform.then(*parent);
    if group.is_plain() {
        draw_children(group, &transform, host, canvas);
        return;
    }
    let object = content_bounds(group, &Transform::IDENTITY);
    let only_clips = group.opacity >= 1.0
        && group.mask.is_none()
        && group.filters.is_empty()
        && group.blend == rikka_vector::BlendMode::SourceOver;
    if only_clips
        && let Some((path, placement, rule)) = group
            .clip
            .as_deref()
            .and_then(|clip| simple_clip(clip, object, &transform))
    {
        match group.paints_once() {
            true => {
                if canvas
                    .push_clip(path, &placement, rule, Contours::Overlapping)
                    .is_ok()
                {
                    draw_children(group, &transform, host, canvas);
                }
                canvas.pop_clip();
            }
            false => draw_clipped(canvas, path, &placement, rule, |canvas| {
                draw_children(group, &transform, host, canvas);
            }),
        }
        return;
    }
    let Some(reach) = layer_bounds(group, &transform) else {
        return;
    };
    match group.filters.is_empty() {
        true => canvas.push_layer_within(covering(reach)),
        false => {
            let (width, height) = (canvas.width() as f32, canvas.height() as f32);
            let surroundings = Rect::new(-width, -height, 3.0 * width, 3.0 * height);
            let Some(reach) = reach.intersection(&surroundings) else {
                return;
            };
            canvas.push_layer_covering(covering(reach));
        }
    }
    let area = canvas.clip_area();
    if area.pixels() == 0 {
        canvas.pop_layer(&Composite::OVER);
        return;
    }
    draw_children(group, &transform, host, canvas);
    for filter in &group.filters {
        filters::apply(filter, object, &transform, host, canvas);
    }
    let mut stencil = group
        .clip
        .as_deref()
        .map(|clip| clip_stencil(clip, object, &transform, host, area));
    if let Some(masking) = group.mask.as_deref() {
        let masked = mask_stencil(masking, object, &transform, host, area);
        stencil = Some(match (stencil, masked) {
            (Some(mut clipped), Some(masked)) => {
                for (value, other) in clipped.data_mut().iter_mut().zip(masked.data()) {
                    *value = ((u32::from(*value) * u32::from(*other) + 127) / 255) as u8;
                }
                clipped
            }
            (None, Some(masked)) => masked,
            (_, None) => {
                canvas.pop_layer(&Composite::faded(0.0));
                return;
            }
        });
    }
    canvas.pop_layer(&Composite {
        mode: group.blend,
        opacity: group.opacity,
        mask: stencil.as_ref().map(|mask| Stencil {
            mask,
            left: area.left,
            top: area.top,
        }),
    });
}

pub(crate) fn draw_clipped(
    canvas: &mut Canvas,
    path: &Path,
    transform: &Transform,
    rule: FillRule,
    draw: impl FnOnce(&mut Canvas),
) {
    if canvas
        .push_clip(path, transform, rule, Contours::Overlapping)
        .is_ok()
    {
        canvas.push_layer();
        draw(canvas);
        canvas.pop_layer(&Composite::OVER);
    }
    canvas.pop_clip();
}

fn layer_bounds(group: &Group, transform: &Transform) -> Option<Rect> {
    if let Some(filter) = group.filters.first() {
        let object = content_bounds(group, &Transform::IDENTITY);
        return bounds::within(filter.rect, filter.units, object)
            .map(|region| bounds::placed(region, transform));
    }
    group.children.iter().fold(None, |total, child| {
        match (total, node_bounds(child, transform, true)) {
            (Some(total), Some(bounds)) => Some(total.union(&bounds)),
            (total, bounds) => total.or(bounds),
        }
    })
}

fn draw_shape(shape: &Shape, parent: &Transform, host: Host, canvas: &mut Canvas) {
    let transform = shape.transform.then(*parent);
    let object = shape.object();
    let room = canvas.width() as usize * canvas.height() as usize;
    canvas.set_antialiasing(shape.antialiasing);
    let fill = |canvas: &mut Canvas| {
        if let Some(fill) = &shape.fill {
            with_paint(
                &fill.ink,
                fill.opacity,
                object,
                &transform,
                host,
                room,
                |paint| {
                    if let Err(error) = canvas.fill(&shape.path, &transform, fill.rule, paint) {
                        tracing::debug!("skipping SVG fill: {error}");
                    }
                },
            );
        }
    };
    let stroke = |canvas: &mut Canvas| {
        if let Some(outline) = &shape.stroke {
            with_paint(
                &outline.ink,
                outline.opacity,
                object,
                &transform,
                host,
                room,
                |paint| {
                    let stroked = match outline.space {
                        StrokeSpace::User => {
                            let mut dashes = Vec::new();
                            let style = style(outline, 1.0, &mut dashes);
                            canvas.stroke(&shape.path, &transform, &style, paint)
                        }
                        StrokeSpace::Host => {
                            match host_outline(&shape.path, &transform, outline, host) {
                                Some(path) => {
                                    canvas.fill(&path, &transform, FillRule::NonZero, paint)
                                }
                                None => Ok(()),
                            }
                        }
                    };
                    if let Err(error) = stroked {
                        tracing::debug!("skipping SVG stroke: {error}");
                    }
                },
            );
        }
    };
    match shape.stroke_first {
        true => {
            stroke(canvas);
            fill(canvas);
        }
        false => {
            fill(canvas);
            stroke(canvas);
        }
    }
    canvas.set_antialiasing(Antialiasing::Enabled);
}

fn style<'d>(outline: &Outline, scale: f32, dashes: &'d mut Vec<f32>) -> Stroke<'d> {
    dashes.clear();
    dashes.extend(outline.dashes.iter().map(|dash| dash * scale));
    Stroke::new(outline.width * scale)
        .join(outline.join)
        .cap(outline.cap)
        .miter_limit(outline.miter_limit)
        .dashed(dashes, outline.dash_offset * scale)
}

fn host_outline(path: &Path, transform: &Transform, outline: &Outline, host: Host) -> Option<Path> {
    let inverse = transform.inverse()?;
    let device = path.transformed(transform);
    let mut dashes = Vec::new();
    let mut stroked = Path::new();
    Stroker::default().outline(
        &device,
        &style(outline, host.scale, &mut dashes),
        FILL_FLATNESS,
        &mut stroked,
    );
    (!stroked.is_truncated()).then(|| stroked.transformed(&inverse))
}

pub(crate) fn fill_rect(rect: Rect) -> (Path, FillRule) {
    let mut path = Path::new();
    path.add_rect(rect);
    (path, FillRule::NonZero)
}
