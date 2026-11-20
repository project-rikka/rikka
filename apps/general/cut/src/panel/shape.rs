use cut_project::{Art, ArtField, Clip, Graphic, Property};
use rikka_app::prelude::*;
use rikka_effects::{Scale, Unit};
use rikka_figure::{Look, Pen, Shape, ShapeKind};

use super::keys::Keying;
use super::outline::outline;
use super::rows::{choice, colour_with, group, sliding_with, switch_row};
use crate::wording;

const WIDEST_LINE: f32 = 500.0;
const FILL: Color = Color::WHITE;
const OUTLINE: Pen = Pen {
    color: Color::BLACK,
    width: 8.0,
};

fn shaped(graphic: &mut Graphic, change: impl FnOnce(&mut Shape, &mut f32, &mut f32, &mut Look)) {
    if let Art::Shape {
        shape,
        width,
        height,
        look,
    } = &mut graphic.art
    {
        change(shape, width, height, look);
    }
}

fn resized(graphic: &mut Graphic, change: &mut dyn FnMut(&mut Shape, &mut f32, &mut f32)) {
    shaped(graphic, |shape, width, height, _| {
        change(shape, width, height)
    });
}

pub(crate) fn shape(art: &Art, clip: &Clip, keying: Keying) -> impl View {
    let Art::Shape {
        shape,
        width,
        height,
        look,
    } = art
    else {
        return group(tr!("cut.panel.shape"), Vec::new());
    };
    let diamond = |field: ArtField| Some(keying.on(clip, Property::Art(field)));
    let pixels = Unit::Pixels;
    let mut rows = vec![choice(
        tr!("cut.panel.kind"),
        ShapeKind::ALL,
        shape.kind(),
        wording::shape,
        |graphic: &mut Graphic, kind: ShapeKind| {
            shaped(graphic, |shape, _, _, _| *shape = Shape::from(kind));
        },
    )];
    rows.extend(outline(*shape, *width, *height, resized, &diamond));
    rows.push(switch_row(
        tr!("cut.panel.fill"),
        look.fill.is_some(),
        |graphic: &mut Graphic, on: bool| {
            shaped(graphic, |_, _, _, look| look.fill = on.then_some(FILL));
        },
    ));
    if let Some(fill) = look.fill {
        rows.push(colour_with(
            wording::art_field(ArtField::Fill, false),
            fill,
            |graphic: &mut Graphic, color: Color| {
                shaped(graphic, |_, _, _, look| look.fill = Some(color));
            },
            diamond(ArtField::Fill),
        ));
    }
    rows.push(switch_row(
        tr!("cut.panel.outline"),
        look.outline.is_some(),
        |graphic: &mut Graphic, on: bool| {
            shaped(graphic, |_, _, _, look| {
                look.outline = on.then_some(OUTLINE)
            });
        },
    ));
    if let Some(pen) = look.outline {
        rows.push(colour_with(
            wording::art_field(ArtField::Stroke, false),
            pen.color,
            |graphic: &mut Graphic, color: Color| {
                shaped(graphic, |_, _, _, look| {
                    if let Some(pen) = &mut look.outline {
                        pen.color = color;
                    }
                });
            },
            diamond(ArtField::Stroke),
        ));
        rows.push(sliding_with(
            wording::art_field(ArtField::StrokeWidth, false),
            pen.width,
            Scale::new(1.0..=WIDEST_LINE).unit(pixels),
            |graphic: &mut Graphic, width: f32| {
                shaped(graphic, |_, _, _, look| {
                    if let Some(pen) = &mut look.outline {
                        pen.width = width;
                    }
                });
            },
            diamond(ArtField::StrokeWidth),
        ));
    }
    group(tr!("cut.panel.shape"), rows)
}
