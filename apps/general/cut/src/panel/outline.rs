use cut_project::ArtField;
use rikka_app::prelude::*;
use rikka_effects::{Scale, Unit};
use rikka_figure::Shape;

use super::rows::{number, number_with, sliding, sliding_with};
use crate::cut::Adjustable;
use crate::wording;

const LARGEST: f32 = 100_000.0;
const ROUNDEST: f32 = 1_000.0;

pub(crate) type Resize<S> = fn(&mut S, &mut dyn FnMut(&mut Shape, &mut f32, &mut f32));

pub(crate) fn outline<S: Adjustable>(
    shape: Shape,
    width: f32,
    height: f32,
    sized: Resize<S>,
    diamond: &dyn Fn(ArtField) -> Option<Element>,
) -> Vec<Element> {
    let pixels = Unit::Pixels;
    let mut rows = vec![
        number_with(
            wording::art_field(ArtField::Width, false),
            width,
            Scale::new(1.0..=LARGEST).unit(pixels),
            move |target: &mut S, value: f32| sized(target, &mut |_, width, _| *width = value),
            diamond(ArtField::Width),
        ),
        number_with(
            wording::art_field(ArtField::Height, false),
            height,
            Scale::new(1.0..=LARGEST).unit(pixels),
            move |target: &mut S, value: f32| sized(target, &mut |_, _, height| *height = value),
            diamond(ArtField::Height),
        ),
    ];
    match shape {
        Shape::Rectangle { radius } if radius > 0.0 => rows.push(sliding_with(
            wording::art_field(ArtField::Corner, false),
            radius,
            Scale::new(1.0..=ROUNDEST).unit(pixels),
            move |target: &mut S, value: f32| {
                sized(target, &mut |shape, _, _| {
                    *shape = Shape::Rectangle { radius: value }
                });
            },
            diamond(ArtField::Corner),
        )),
        Shape::Polygon { sides } => rows.push(number(
            tr!("cut.panel.sides"),
            sides as f32,
            Scale::new(Shape::FEWEST_SIDES as f32..=Shape::MAX_SIDES as f32),
            move |target: &mut S, value: f32| {
                sized(target, &mut |shape, _, _| {
                    *shape = Shape::Polygon {
                        sides: value.round() as u32,
                    };
                });
            },
        )),
        Shape::Star { points, depth } => {
            rows.push(number(
                tr!("cut.panel.points"),
                points as f32,
                Scale::new(Shape::FEWEST_SIDES as f32..=Shape::MAX_SIDES as f32),
                move |target: &mut S, value: f32| {
                    sized(target, &mut |shape, _, _| {
                        if let Shape::Star { depth, .. } = *shape {
                            *shape = Shape::Star {
                                points: value.round() as u32,
                                depth,
                            };
                        }
                    });
                },
            ));
            rows.push(sliding(
                tr!("cut.panel.depth"),
                depth * 100.0,
                Scale::new(Shape::SHALLOWEST * 100.0..=Shape::DEEPEST * 100.0).unit(Unit::Percent),
                move |target: &mut S, value: f32| {
                    sized(target, &mut |shape, _, _| {
                        if let Shape::Star { points, .. } = *shape {
                            *shape = Shape::Star {
                                points,
                                depth: value / 100.0,
                            };
                        }
                    });
                },
            ));
        }
        Shape::Rectangle { .. } | Shape::Ellipse | Shape::Triangle => {}
    }
    rows
}
