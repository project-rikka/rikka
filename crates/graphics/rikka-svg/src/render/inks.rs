use rikka_types::{Color, Rect};
use rikka_vector::{
    Canvas, Gradient as Shading, Interpolation, Paint, Pattern as Tiles, Pixmap, Sampling, Stop,
    Tiling, Transform,
};

use crate::parse::values::fit;
use crate::render::bounds::{scales, unit_box, within};
use crate::render::{Host, draw_children};
use crate::tree::{Gradient, Ink, Pattern, Units};

const MAX_TILE_SIDE: f32 = 4096.0;

pub(crate) fn with_paint(
    ink: &Ink,
    opacity: f32,
    object: Option<Rect>,
    transform: &Transform,
    host: Host,
    room: usize,
    draw: impl FnOnce(&Paint<'_>),
) {
    match ink {
        Ink::Color(color) => draw(&Paint::Solid(faded(*color, opacity))),
        Ink::Gradient(gradient) => with_gradient(gradient, opacity, object, draw),
        Ink::Pattern(pattern) => {
            if let Some((pixmap, placement)) = tile(pattern, object, transform, host, room) {
                draw(&Paint::Pattern(Tiles {
                    pixmap: &pixmap,
                    transform: placement,
                    tiling: Tiling::Repeat,
                    sampling: Sampling::Bilinear,
                    opacity,
                }));
            }
        }
    }
}

fn faded(color: Color, opacity: f32) -> Color {
    Color::rgba(color.r, color.g, color.b, color.a * opacity)
}

fn with_gradient(
    gradient: &Gradient,
    opacity: f32,
    object: Option<Rect>,
    draw: impl FnOnce(&Paint<'_>),
) {
    let placement = match gradient.units {
        Units::UserSpace => gradient.transform,
        Units::BoundingBox => {
            match object.filter(|object| object.width() > 0.0 && object.height() > 0.0) {
                Some(object) => gradient.transform.then(unit_box(object)),
                None => return,
            }
        }
    };
    let faded_stops: Vec<Stop>;
    let stops = match opacity < 1.0 {
        true => {
            faded_stops = gradient
                .stops
                .iter()
                .map(|stop| Stop {
                    offset: stop.offset,
                    color: faded(stop.color, opacity),
                })
                .collect();
            &faded_stops
        }
        false => &gradient.stops,
    };
    draw(&Paint::Gradient(Shading {
        shape: gradient.shape,
        stops,
        spread: gradient.spread,
        interpolation: Interpolation::Srgb,
        transform: placement,
    }));
}

fn tile(
    pattern: &Pattern,
    object: Option<Rect>,
    transform: &Transform,
    host: Host,
    room: usize,
) -> Option<(Pixmap, Transform)> {
    let rect = within(pattern.rect, pattern.units, object)?;
    if !(rect.width() > 0.0 && rect.height() > 0.0) {
        return None;
    }
    let scale = scales(&pattern.transform.then(*transform));
    let mut width = (rect.width() * scale.x).ceil().clamp(1.0, MAX_TILE_SIDE);
    let mut height = (rect.height() * scale.y).ceil().clamp(1.0, MAX_TILE_SIDE);
    let shrink = (room as f32 / (width * height)).sqrt();
    if shrink < 1.0 {
        width = (width * shrink).floor().max(1.0);
        height = (height * shrink).floor().max(1.0);
    }
    let (across, down) = (width / rect.width(), height / rect.height());
    let content = match (pattern.view, pattern.content_units) {
        (Some((view, aspect)), _) => fit(view, aspect, rect.width(), rect.height()),
        (None, Units::BoundingBox) => {
            let object = object?;
            Transform::scale(object.width(), object.height())
        }
        (None, Units::UserSpace) => Transform::IDENTITY,
    };
    let mut canvas = Canvas::new();
    canvas.begin(width as u32, height as u32);
    draw_children(
        &pattern.content,
        &content.then(Transform::scale(across, down)),
        host,
        &mut canvas,
    );
    let placement = Transform::scale(1.0 / across, 1.0 / down)
        .then(Transform::translate(rect.min_x(), rect.min_y()))
        .then(pattern.transform);
    Some((canvas.to_pixmap().ok()?, placement))
}
