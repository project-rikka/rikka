use animate_cast::{MmdAxis, Skeleton};
use animate_stage::{Projection, Space, project};
use animate_ui::{centred, dotted};
use rikka_app::prelude::*;

use crate::handles::{Handle, Layout, Offer, Row};

const MARKER: f32 = 9.0;
const REACH: f32 = 8.0;
const OUTLINE: f32 = 1.5;
const LETTER: f32 = 14.0;
const LETTER_LINE: f32 = 18.0;
const NOTE: f32 = 11.5;
const NOTE_LINE: f32 = 16.0;
const GLYPH: f32 = 18.0;
const DIMMED: f32 = 0.3;

const MOVABLE: Color = Color::rgb(0.25, 0.55, 1.0);
const IK: Color = Color::rgb(1.0, 0.55, 0.1);
const CHOSEN: Color = Color::rgb(1.0, 0.2, 0.2);
pub(super) const AXES: [Color; 3] = [
    Color::rgb(0.95, 0.3, 0.3),
    Color::rgb(0.35, 0.85, 0.35),
    Color::rgb(0.35, 0.55, 1.0),
];

fn spots<'a>(
    projection: &'a Projection,
    skeleton: &'a Skeleton,
    frame: Rect,
) -> impl Iterator<Item = (usize, Point)> + 'a {
    skeleton
        .bones()
        .iter()
        .enumerate()
        .filter(|(_, bone)| bone.editable())
        .filter_map(move |(index, _)| {
            let world = projection.worlds.get(index)?;
            let at = project(projection.view_projection, frame, world.w_axis.truncate())?;
            frame.contains(at).then_some((index, at))
        })
}

pub(super) fn bone_at(
    projection: &Projection,
    skeleton: &Skeleton,
    frame: Rect,
    at: Point,
) -> Option<usize> {
    spots(projection, skeleton, frame)
        .map(|(index, spot)| (index, (spot.x - at.x).hypot(spot.y - at.y)))
        .filter(|(_, away)| *away <= REACH)
        .min_by(|one, two| one.1.total_cmp(&two.1))
        .map(|(index, _)| index)
}

pub(super) fn paint_bones(
    ctx: &mut PaintCtx<'_>,
    projection: &Projection,
    skeleton: &Skeleton,
    frame: Rect,
    chosen: Option<usize>,
) {
    let spots: Vec<(usize, Point)> = spots(projection, skeleton, frame).collect();
    let place = |index: usize| {
        spots
            .iter()
            .find(|(candidate, _)| *candidate == index)
            .map(|(_, at)| *at)
    };
    ctx.clipped(frame, |ctx| {
        for (index, at) in &spots {
            let Some(parent) = skeleton.bone(*index).and_then(|bone| bone.parent) else {
                continue;
            };
            if let Some(from) = place(parent) {
                dotted(ctx, from, *at, MOVABLE.scale_alpha(0.6));
            }
        }
        for (index, at) in &spots {
            let Some(bone) = skeleton.bone(*index) else {
                continue;
            };
            let tint = match (chosen == Some(*index), bone.reaches) {
                (true, _) => CHOSEN,
                (false, true) => IK,
                (false, false) => MOVABLE,
            };
            let marker = Rect::from_center(*at, Size::new(MARKER, MARKER));
            let radius = match bone.moves {
                true => 1.0,
                false => MARKER / 2.0,
            };
            let fill = match chosen == Some(*index) {
                true => tint,
                false => tint.scale_alpha(0.25),
            };
            ctx.draw_quad(
                Quad::new(marker)
                    .fill(fill)
                    .stroke(OUTLINE, tint)
                    .radius(radius),
            );
        }
    });
}

pub(super) struct Lettering {
    pub(super) global: Label,
    pub(super) local: Label,
}

pub(super) fn paint_handles(
    ctx: &mut PaintCtx<'_>,
    layout: &Layout,
    offer: Offer,
    space: Space,
    lit: Option<Handle>,
    lettering: &Lettering,
) {
    let palette = palette();
    for row in [Row::Turn, Row::Slide] {
        let symbol = match row {
            Row::Turn => MaterialSymbol::ROTATE_RIGHT,
            Row::Slide => MaterialSymbol::OPEN_WITH,
        };
        let open = match row {
            Row::Turn => offer.allows(Handle::Turn(MmdAxis::X)),
            Row::Slide => offer.allows(Handle::Slide(MmdAxis::X)),
        };
        let sign = layout.sign(row);
        ctx.draw_icon(
            Rect::from_center(sign.center(), Size::new(GLYPH, GLYPH)),
            IconGlyph::from(symbol).spec(GLYPH),
            dimmed(palette.text, open),
        );
    }
    for handle in Layout::all() {
        let open = offer.allows(handle);
        if handle == Handle::Space && !open {
            continue;
        }
        let rect = layout.rect(handle);
        let ground = match (lit == Some(handle) && open, open) {
            (true, _) => palette.accent.scale_alpha(0.35),
            (false, true) => palette.surface.scale_alpha(0.75),
            (false, false) => palette.surface.scale_alpha(0.35),
        };
        ctx.draw_quad(
            Quad::new(rect)
                .fill(ground)
                .stroke(1.0, palette.outline.scale_alpha(0.6))
                .radius(Corner::Item),
        );
        match handle {
            Handle::Turn(axis) | Handle::Slide(axis) => {
                let (letter, tint) = (axis.letter(), AXES[axis as usize]);
                centred(
                    ctx,
                    rect,
                    TextRun::new(letter, LETTER).weight(Weight::BOLD),
                    LETTER_LINE,
                    dimmed(tint, open),
                );
            }
            Handle::Space => {
                let label = match space {
                    Space::Global => &lettering.global,
                    Space::Local => &lettering.local,
                };
                centred(
                    ctx,
                    rect,
                    TextRun::new(label.as_str(), NOTE).medium(),
                    NOTE_LINE,
                    palette.text,
                );
            }
        }
    }
}

fn dimmed(tint: Color, open: bool) -> Color {
    match open {
        true => tint,
        false => tint.scale_alpha(DIMMED),
    }
}
