use cut_project::{Area, Region, Shot, Wipe};
use rikka_figure::{Frame, Shape};
use rikka_image_edit::{FillRule, Mask, Path};
use rikka_types::{Point, Rect};

use crate::geometry::centred;
use crate::plan::Layer;
use crate::request::Request;
use crate::sight::Sight;
use crate::stack::Stack;
use crate::wiping::{clip_to_wipes, wipes};

enum Coverage {
    Whole,
    Within(Path),
    Nowhere,
}

pub(crate) fn filter(
    request: &Request,
    shot: &Shot,
    region: &Region,
    stack: &Stack,
) -> Option<Layer> {
    let placement = region.placement;
    let opacity = placement.opacity;
    if stack.is_empty() || opacity <= 0.0 {
        return None;
    }
    let factor = request.factor();
    let camera = shot
        .path
        .stretch(Point::new(placement.x, placement.y))
        .unwrap_or(1.0);
    let wipes = wipes(&shot.transform, false);
    let mask = match region.area {
        Area::Whole => match whole(request, shot, &wipes) {
            Coverage::Whole => None,
            Coverage::Within(path) => Some(Mask::path(path, FillRule::NonZero)),
            Coverage::Nowhere => return None,
        },
        Area::Shape {
            shape,
            width,
            height,
        } => match (
            shaped(request, shot, shape, centred(width, height), &wipes),
            region.inverted,
        ) {
            (Coverage::Whole, _) | (Coverage::Nowhere, true) => None,
            (Coverage::Nowhere, false) => return None,
            (Coverage::Within(path), inverted) => {
                let feather = (region.feather + shot.transform.blur) * factor * camera;
                let mask = Mask::path(path, FillRule::NonZero).feathered(feather);
                Some(match inverted {
                    true => mask.inverted(),
                    false => mask,
                })
            }
        },
    };
    Some(Layer::Filter {
        steps: stack.scaled(factor * camera),
        mask,
        opacity,
    })
}

fn shaped(request: &Request, shot: &Shot, shape: Shape, local: Rect, wipes: &[Wipe]) -> Coverage {
    let Some(sight) = Sight::new(
        &shot.projection,
        local,
        request.project().format(),
        request.factor(),
    ) else {
        return Coverage::Nowhere;
    };
    let fine = sight.stretch();
    let shape = match shape {
        Shape::Rectangle { radius } => Shape::Rectangle {
            radius: radius * fine,
        },
        other => other,
    };
    let path = shape.path(&Frame::new(
        Point::ZERO,
        local.width() * fine,
        local.height() * fine,
    ));
    let mut builder = Path::builder();
    let mut any = false;
    for subpath in path.subpaths() {
        let polygon: Vec<Point> = subpath
            .points()
            .iter()
            .map(|point| *point * (1.0 / fine))
            .collect();
        let Some(points) = sight.traced(&clip_to_wipes(wipes, polygon, local)) else {
            continue;
        };
        any = true;
        for (at, point) in points.into_iter().enumerate() {
            builder = match at {
                0 => builder.move_to(point),
                _ => builder.line_to(point),
            };
        }
        builder = builder.close();
    }
    match any {
        true => Coverage::Within(builder.build()),
        false => Coverage::Nowhere,
    }
}

fn whole(request: &Request, shot: &Shot, wipes: &[Wipe]) -> Coverage {
    let format = request.project().format();
    let frame = centred(format.width as f32, format.height as f32);
    if wipes.is_empty() && covers(request, shot, frame) {
        return Coverage::Whole;
    }
    let Some(sight) = Sight::new(&shot.path, frame, format, request.factor()) else {
        return Coverage::Nowhere;
    };
    match sight.traced(&clip_to_wipes(wipes, frame.corners().to_vec(), frame)) {
        Some(points) => Coverage::Within(Path::polygon(points)),
        None => Coverage::Nowhere,
    }
}

fn covers(request: &Request, shot: &Shot, frame: Rect) -> bool {
    let (width, height) = request.output();
    let factor = request.factor();
    let bounds = Rect::new(0.0, 0.0, width as f32 / factor, height as f32 / factor);
    let room = frame.inflate(1.0 / factor);
    bounds.corners().iter().all(|corner| {
        shot.path
            .unmap(*corner)
            .is_some_and(|local| room.contains(local))
    })
}
