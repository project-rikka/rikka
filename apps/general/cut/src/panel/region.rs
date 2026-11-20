use cut_project::{Area, Format, Region};
use rikka_app::prelude::*;
use rikka_effects::{Scale, Unit};
use rikka_figure::{Shape, ShapeKind};

use super::outline::outline;
use super::rows::{choice, group, sliding, switch_row};
use crate::{art, wording};

const AREAS: [Option<ShapeKind>; 7] = [
    None,
    Some(ShapeKind::Rectangle),
    Some(ShapeKind::RoundedRectangle),
    Some(ShapeKind::Ellipse),
    Some(ShapeKind::Triangle),
    Some(ShapeKind::Polygon),
    Some(ShapeKind::Star),
];

fn reshaped(area: Area, kind: Option<ShapeKind>, format: Format) -> Area {
    match (kind, area) {
        (None, _) => Area::Whole,
        (Some(kind), Area::Shape { width, height, .. }) => Area::Shape {
            shape: Shape::from(kind),
            width,
            height,
        },
        (Some(kind), Area::Whole) => art::area(kind, format),
    }
}

fn sized(region: &mut Region, change: &mut dyn FnMut(&mut Shape, &mut f32, &mut f32)) {
    if let Area::Shape {
        shape,
        width,
        height,
    } = &mut region.area
    {
        change(shape, width, height);
    }
}

pub(crate) fn area(region: &Region, format: Format) -> impl View {
    let selected = match region.area {
        Area::Whole => None,
        Area::Shape { shape, .. } => Some(shape.kind()),
    };
    let mut rows = vec![choice(
        tr!("cut.panel.kind"),
        AREAS,
        selected,
        wording::area,
        move |region: &mut Region, kind: Option<ShapeKind>| {
            region.area = reshaped(region.area, kind, format);
        },
    )];
    if let Area::Shape {
        shape,
        width,
        height,
    } = region.area
    {
        rows.extend(outline(shape, width, height, sized, &|_| None));
        rows.push(sliding(
            tr!("cut.panel.feather"),
            region.feather,
            Scale::new(Region::FEATHERS).unit(Unit::Pixels),
            |region: &mut Region, feather: f32| region.feather = feather,
        ));
        rows.push(switch_row(
            tr!("cut.panel.inverted"),
            region.inverted,
            |region: &mut Region, on: bool| region.inverted = on,
        ));
    }
    group(tr!("cut.panel.range"), rows)
}
