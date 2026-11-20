use cut_project::{Clip, Graphic, Placement, PlacementField, Property, Region, Treatment};
use rikka_app::prelude::*;
use rikka_effects::{Scale, Unit};

use super::keys::Keying;
use super::rows::{group_with, number_with, pair_with, sliding_with, switch_row};
use crate::cut::Adjustable;
use crate::wording;

const FARTHEST: f32 = 100_000.0;
const MAX_ZOOM: f32 = 400.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Row {
    Position,
    Anchor,
    Zoom,
    Aspect,
    Rotation,
    ThreeD,
    Depth,
    TiltX,
    TiltY,
    Opacity,
}

impl Row {
    pub(crate) const ALL: [Row; 10] = [
        Row::Position,
        Row::Anchor,
        Row::Zoom,
        Row::Aspect,
        Row::Rotation,
        Row::ThreeD,
        Row::Depth,
        Row::TiltX,
        Row::TiltY,
        Row::Opacity,
    ];

    fn field(self) -> Option<PlacementField> {
        match self {
            Row::Position => Some(PlacementField::Position),
            Row::Anchor => Some(PlacementField::Anchor),
            Row::Zoom => Some(PlacementField::Zoom),
            Row::Aspect => Some(PlacementField::Aspect),
            Row::Rotation => Some(PlacementField::Rotation),
            Row::ThreeD => None,
            Row::Depth => Some(PlacementField::Depth),
            Row::TiltX => Some(PlacementField::TiltX),
            Row::TiltY => Some(PlacementField::TiltY),
            Row::Opacity => Some(PlacementField::Opacity),
        }
    }

    fn is_visible(self, three_d: bool) -> bool {
        three_d || self.field().is_none_or(|field| !field.needs_three_d())
    }
}

pub(crate) trait HasPlacement: Adjustable {
    fn placement_mut(&mut self) -> &mut Placement;
}

impl HasPlacement for Graphic {
    fn placement_mut(&mut self) -> &mut Placement {
        &mut self.placement
    }
}

impl HasPlacement for Treatment {
    fn placement_mut(&mut self) -> &mut Placement {
        &mut self.placement
    }
}

impl HasPlacement for Region {
    fn placement_mut(&mut self) -> &mut Placement {
        &mut self.placement
    }
}

pub(crate) fn placement<S: HasPlacement>(
    clip: &Clip,
    placement: Placement,
    keying: Keying,
    rows: &[Row],
) -> impl View {
    let rows = rows
        .iter()
        .filter(|entry| entry.is_visible(placement.three_d))
        .map(|&entry| {
            let diamond = entry
                .field()
                .map(|field| keying.on(clip, Property::Placement(field)));
            row::<S>(entry, placement, diamond)
        })
        .collect();
    group_with(tr!("cut.panel.drawing"), Some(keying.group(clip)), rows)
}

fn pixels() -> Scale {
    Scale::new(-FARTHEST..=FARTHEST)
        .unit(Unit::Pixels)
        .decimals(1)
}

fn degrees() -> Scale {
    Scale::new(-180.0..=180.0).unit(Unit::Degrees).decimals(1)
}

fn row<S: HasPlacement>(kind: Row, placement: Placement, diamond: Option<Element>) -> Element {
    match kind {
        Row::Position => pair_with(
            wording::placement_field(PlacementField::Position),
            placement.position(),
            pixels(),
            (
                |placed: &mut S, x: f32| placed.placement_mut().x = x,
                |placed: &mut S, y: f32| placed.placement_mut().y = y,
            ),
            diamond,
        ),
        Row::Anchor => pair_with(
            wording::placement_field(PlacementField::Anchor),
            placement.anchor,
            pixels(),
            (
                |placed: &mut S, x: f32| placed.placement_mut().anchor.x = x,
                |placed: &mut S, y: f32| placed.placement_mut().anchor.y = y,
            ),
            diamond,
        ),
        Row::Zoom => sliding_with(
            wording::placement_field(PlacementField::Zoom),
            placement.zoom * 100.0,
            Scale::new(Placement::MIN_ZOOM * 100.0..=MAX_ZOOM)
                .unit(Unit::Percent)
                .decimals(1),
            |placed: &mut S, zoom: f32| placed.placement_mut().zoom = zoom / 100.0,
            diamond,
        ),
        Row::Aspect => sliding_with(
            wording::placement_field(PlacementField::Aspect),
            placement.aspect * 100.0,
            Scale::new(-100.0..=100.0).unit(Unit::Percent).decimals(1),
            |placed: &mut S, aspect: f32| placed.placement_mut().aspect = aspect / 100.0,
            diamond,
        ),
        Row::Rotation => sliding_with(
            wording::placement_field(PlacementField::Rotation),
            placement.rotation,
            degrees(),
            |placed: &mut S, rotation: f32| placed.placement_mut().rotation = rotation,
            diamond,
        ),
        Row::ThreeD => switch_row(
            tr!("cut.rows.three_d"),
            placement.three_d,
            |placed: &mut S, on: bool| placed.placement_mut().three_d = on,
        ),
        Row::Depth => number_with(
            wording::placement_field(PlacementField::Depth),
            placement.depth,
            pixels(),
            |placed: &mut S, depth: f32| placed.placement_mut().depth = depth,
            diamond,
        ),
        Row::TiltX => sliding_with(
            wording::placement_field(PlacementField::TiltX),
            placement.tilt_x,
            degrees(),
            |placed: &mut S, tilt: f32| placed.placement_mut().tilt_x = tilt,
            diamond,
        ),
        Row::TiltY => sliding_with(
            wording::placement_field(PlacementField::TiltY),
            placement.tilt_y,
            degrees(),
            |placed: &mut S, tilt: f32| placed.placement_mut().tilt_y = tilt,
            diamond,
        ),
        Row::Opacity => sliding_with(
            wording::placement_field(PlacementField::Opacity),
            placement.opacity * 100.0,
            Scale::new(0.0..=100.0).unit(Unit::Percent),
            |placed: &mut S, opacity: f32| placed.placement_mut().opacity = opacity / 100.0,
            diamond,
        ),
    }
}
