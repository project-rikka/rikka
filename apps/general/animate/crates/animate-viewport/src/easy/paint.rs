use animate_body::Side;
use animate_hold::Holder;
use animate_solve::Handle;
use animate_stage::{Lens, Spot};
use animate_ui::{disc, stroke};
use rikka_app::prelude::*;
use rikka_model::glam::Vec3;

use crate::easy::drag::EasyDragState;
use crate::easy::grasp::Grasping;
use crate::easy::part::{Marker, part_of};
use crate::easy::paths::path;
use crate::easy::scene::{Control, Scene, Sight};
use crate::easy::style::{
    BAND, CHOSEN_GAP, CONTROL_HALO, CONTROL_OUTLINE, CROSSHAIR, GAZE, HELD, LEFT, LINE, LINKS,
    LIT_RING, MARKER, NAME, NAME_GAP, NAME_LINE, PATCH_DOT, PATH, PATH_DOT, PATH_KEY, PATH_STEP,
    PIN_DOT, PIN_OUTLINE, PINNED, PUPIL, RIGHT, RING, RING_LINE, RING_LIT_LINE, RING_REACH,
    SNAPPED, TARGET, radius, square, tint,
};
use crate::wording;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Resting,
    Lit,
    Grabbed,
}

fn dots(ctx: &mut PaintCtx<'_>, spots: &[Point], spacing: f32, size: f32, tint: Color) {
    let mut carried = 0.0;
    for pair in spots.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let length = from.distance(to);
        let mut along = spacing - carried;
        while along <= length {
            ctx.draw_circle(from + (to - from) * (along / length.max(1e-6)), size, tint);
            along += spacing;
        }
        carried = (carried + length) % spacing;
    }
}

fn shape(at: Point, size: f32, squared: bool) -> Quad {
    match squared {
        true => Quad::new(Rect::from_center(at, Size::new(size * 2.0, size * 2.0))).radius(2.0),
        false => disc(at, size),
    }
}

fn control(ctx: &mut PaintCtx<'_>, control: &Control, look: Look, chosen: bool) {
    let size = radius(control.marker);
    let squared = square(control.marker);
    let hue = tint(control.marker);
    let (fill, rim) = match (look, control.pinned) {
        (Look::Grabbed, _) => (HELD, HELD),
        (Look::Lit, _) => (hue.scale_alpha(0.85), hue),
        (Look::Resting, true) => (hue.scale_alpha(0.25), PINNED),
        (Look::Resting, false) => (hue.scale_alpha(0.25), hue),
    };
    let outline = match control.pinned {
        true => PIN_OUTLINE,
        false => CONTROL_OUTLINE,
    };
    ctx.draw_quad(
        shape(control.at, size + CONTROL_HALO, squared).fill(Color::BLACK.scale_alpha(0.35)),
    );
    ctx.draw_quad(
        shape(control.at, size, squared)
            .fill(fill)
            .stroke(outline, rim),
    );
    if control.pinned {
        ctx.draw_quad(disc(control.at, size * PIN_DOT).fill(PINNED));
    }
    feature(ctx, control.marker, control.at, size, rim);
    if look == Look::Lit {
        ctx.draw_ring(Ring::new(control.at, (size + LIT_RING) * 2.0).stroke(LINE, Color::WHITE));
    }
    if chosen {
        let reach = (size + CHOSEN_GAP) * 2.0;
        ctx.draw_ring(Ring::new(control.at, reach).stroke(CONTROL_OUTLINE, Color::WHITE));
    }
}

fn feature(ctx: &mut PaintCtx<'_>, marker: Marker, at: Point, size: f32, rim: Color) {
    match marker {
        Marker::Eyes => ctx.draw_quad(disc(at, size * PUPIL).fill(rim)),
        Marker::Mouth => {
            let half = Point::new(size * PUPIL, 0.0);
            stroke(ctx, at - half, at + half, LINE * 0.6, rim);
        }
        Marker::Handle(_) | Marker::Finger(_) => {}
    }
}

fn links(ctx: &mut PaintCtx<'_>, sight: &Sight) {
    for control in &sight.controls {
        let mut parent = control.marker.parent();
        while let Some(marker) = parent {
            if let Some(above) = sight.find(marker) {
                stroke(ctx, above.at, control.at, LINE, LINKS.scale_alpha(0.6));
                break;
            }
            parent = marker.parent();
        }
    }
}

fn name(ctx: &mut PaintCtx<'_>, control: &Control) {
    let label = wording::marker(control.marker);
    caption(ctx, control.at, radius(control.marker), label.as_str());
}

fn caption(ctx: &mut PaintCtx<'_>, at: Point, size: f32, text: &str) {
    let run = TextRun::new(text, NAME).medium().no_wrap();
    let width = measure_text(&run, None).width + 2.0;
    let at = Point::new(at.x + size + NAME_GAP, at.y - size - NAME_LINE);
    let shadow = Rect::new(at.x + 1.0, at.y + 1.0, width, NAME_LINE);
    ctx.draw_text(shadow, run.clone(), Color::BLACK.scale_alpha(0.8));
    ctx.draw_text(Rect::new(at.x, at.y, width, NAME_LINE), run, Color::WHITE);
}

fn rings(ctx: &mut PaintCtx<'_>, scene: &Scene, sight: &Sight, drag: &EasyDragState) {
    let highlighted = drag.lit_ring();
    for &(side, at) in &sight.rings {
        let hue = match scene.grip_of(side) {
            Some(_) => HELD,
            None => tint(Marker::Handle(Handle::Hand(side))),
        };
        let (width, shade) = match highlighted == Some(side) {
            true => (RING_LIT_LINE, hue),
            false => (RING_LINE, hue.scale_alpha(0.6)),
        };
        let halo = Color::BLACK.scale_alpha(0.35);
        ctx.draw_ring(Ring::new(at, RING * 2.0).stroke(width + CONTROL_HALO * 2.0, halo));
        ctx.draw_ring(Ring::new(at, RING * 2.0).stroke(width, shade));
        if highlighted == Some(side) && drag.grasping().is_none() {
            caption(ctx, at, RING, wording::grasp(side).as_str());
        }
    }
}

pub(crate) fn paint(ctx: &mut PaintCtx<'_>, scene: &Scene, drag: &EasyDragState) {
    let lens = scene.lens;
    let body = &scene.body;
    let sight = scene.projection();
    ctx.clipped(lens.frame(), |ctx| {
        links(ctx, &sight);
        rings(ctx, scene, &sight, drag);

        if scene.paths()
            && let (Some(part), Some(motion_bake), Some(human)) = (
                scene.part,
                scene.motion_bake.as_ref(),
                scene.take.human.as_ref(),
            )
            && let Some(path) = path(body, human, motion_bake, part, scene.frame)
        {
            let spots: Vec<Point> = path
                .points
                .iter()
                .filter_map(|point| lens.project(*point))
                .collect();
            dots(ctx, &spots, PATH_STEP, PATH_DOT, PATH);
            for key in path.keys.iter().filter_map(|key| lens.project(*key)) {
                ctx.draw_quad(
                    Quad::new(Rect::from_center(key, Size::new(PATH_KEY, PATH_KEY)))
                        .fill(PATH)
                        .radius(PATH_KEY / 2.0),
                );
            }
        }

        let highlighted = drag.lit();
        for marker_control in &sight.controls {
            let look = match (highlighted == Some(marker_control.marker), drag.holding()) {
                (true, true) => Look::Grabbed,
                (true, false) => Look::Lit,
                (false, _) => Look::Resting,
            };
            let chosen = scene.part.is_some() && scene.part == part_of(marker_control.marker);
            control(ctx, marker_control, look, chosen && look == Look::Resting);
        }
        if let Some(control) = highlighted.and_then(|marker| sight.find(marker)) {
            name(ctx, control);
        }

        if let Some((_, at)) = sight.gaze {
            let half = CROSSHAIR / 2.0;
            stroke(
                ctx,
                at - Point::new(half, 0.0),
                at + Point::new(half, 0.0),
                LINE,
                GAZE,
            );
            stroke(
                ctx,
                at - Point::new(0.0, half),
                at + Point::new(0.0, half),
                LINE,
                GAZE,
            );
            ctx.draw_ring(Ring::new(at, CROSSHAIR * 0.7).stroke(LINE, GAZE));
        }

        holding(ctx, scene);
        if let Some(grasping) = drag.grasping() {
            reaching(ctx, scene, &sight, grasping);
        }
    });
}

fn holding(ctx: &mut PaintCtx<'_>, scene: &Scene) {
    let lens = &scene.lens;
    for grip in &scene.grips {
        let Some(at) = lens.project(grip.point) else {
            continue;
        };
        let hue = match grip.holder {
            Holder::Hand(Side::Left) => LEFT,
            Holder::Hand(Side::Right) => RIGHT,
            Holder::Pin { .. } => PINNED,
        };
        ctx.draw_quad(disc(at, MARKER + CONTROL_HALO).fill(Color::BLACK.scale_alpha(0.35)));
        ctx.draw_quad(disc(at, MARKER).fill(HELD).stroke(CONTROL_OUTLINE, hue));
    }
}

fn reaching(ctx: &mut PaintCtx<'_>, scene: &Scene, sight: &Sight, grasping: &Grasping) {
    let lens = &scene.lens;
    let Some(ring) = sight.ring(grasping.side()) else {
        return;
    };
    let spot = grasping.spot();
    let members = match (spot, &scene.cloth_snapshot) {
        (Some(spot), Some(cloth_snapshot)) => cloth_snapshot.patch(spot, grasping.reach()),
        _ => Vec::new(),
    };
    let caught = !members.is_empty();
    let end = spot
        .and_then(|spot| lens.project(spot.world))
        .unwrap_or(grasping.pointer());
    let shade = match caught {
        true => SNAPPED,
        false => SNAPPED.scale_alpha(0.45),
    };
    let length = ring.distance(end);
    if length > RING {
        let start = ring + (end - ring) * (RING / length);
        stroke(ctx, start, end, BAND, shade);
    }
    patch(ctx, lens, &members);
    if spot.is_none() && length <= RING + RING_REACH {
        return;
    }
    ctx.draw_ring(Ring::new(end, TARGET * 2.0).stroke(RING_LINE, shade));
    ctx.draw_quad(disc(end, TARGET * 0.35).fill(shade));
    if let Some(spot) = spot.filter(|_| caught) {
        caption(ctx, end, TARGET, &spot_name(scene, spot));
    }
}

fn spot_name(scene: &Scene, spot: &Spot) -> String {
    match spot.figure == scene.id {
        true => spot.piece.clone(),
        false => format!("{} / {}", spot.name, spot.piece),
    }
}

fn patch(ctx: &mut PaintCtx<'_>, lens: &Lens, members: &[(Vec3, f32)]) {
    for (world, weight) in members {
        if let Some(at) = lens.project(*world) {
            ctx.draw_circle(at, PATCH_DOT, SNAPPED.scale_alpha(0.25 + 0.75 * weight));
        }
    }
}
