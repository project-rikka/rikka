use cut_project::{Art, ArtField, Clip, Graphic, Property};
use rikka_app::prelude::*;
use rikka_effects::{Scale, Unit};
use rikka_figure::Pen;

use super::keys::Keying;
use super::rows::{colour_with, group, pair_with, sliding_with, switch_row};
use crate::wording;

const WIDEST: f32 = 500.0;
const FARTHEST: f32 = 100_000.0;

fn lined(graphic: &mut Graphic, change: impl FnOnce(&mut Point, &mut Pen, &mut bool)) {
    if let Art::Line { reach, pen, arrow } = &mut graphic.art {
        change(reach, pen, arrow);
    }
}

pub(crate) fn line(art: &Art, clip: &Clip, keying: Keying) -> impl View {
    let Art::Line { reach, pen, arrow } = art else {
        return group(tr!("cut.panel.line"), Vec::new());
    };
    let diamond = |field: ArtField| Some(keying.on(clip, Property::Art(field)));
    group(
        tr!("cut.panel.line"),
        vec![
            pair_with(
                wording::art_field(ArtField::End, true),
                *reach,
                Scale::new(-FARTHEST..=FARTHEST)
                    .unit(Unit::Pixels)
                    .decimals(1),
                (
                    |graphic: &mut Graphic, x: f32| lined(graphic, |reach, _, _| reach.x = x),
                    |graphic: &mut Graphic, y: f32| lined(graphic, |reach, _, _| reach.y = y),
                ),
                diamond(ArtField::End),
            ),
            colour_with(
                wording::art_field(ArtField::Stroke, true),
                pen.color,
                |graphic: &mut Graphic, color: Color| {
                    lined(graphic, |_, pen, _| pen.color = color);
                },
                diamond(ArtField::Stroke),
            ),
            sliding_with(
                wording::art_field(ArtField::StrokeWidth, true),
                pen.width,
                Scale::new(1.0..=WIDEST).unit(Unit::Pixels),
                |graphic: &mut Graphic, width: f32| lined(graphic, |_, pen, _| pen.width = width),
                diamond(ArtField::StrokeWidth),
            ),
            switch_row(
                tr!("cut.panel.arrow"),
                *arrow,
                |graphic: &mut Graphic, on: bool| lined(graphic, |_, _, arrow| *arrow = on),
            ),
        ],
    )
}
