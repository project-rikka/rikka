use std::ops::Range;

use cut_project::{
    Clip, Edge, EditPoint, FrozenFrames, Project, TransitionAlignment, TransitionKind,
};
use rikka_app::prelude::*;

use crate::timeline::rows::{Row, Rows};
use crate::timeline::scale::Scale;
use crate::wording;

pub(crate) const BAR: f32 = 14.0;
const INSET: f32 = 2.0;
const NARROWEST: f32 = 8.0;
const GRIP: f32 = 5.0;
const REACH: f32 = 8.0;
const LANDING: f32 = 28.0;
const CENTRE: f32 = 6.0;
const NAME: f32 = 10.0;
const FILL: f32 = 0.88;
const STRIPE: f32 = 4.0;
const STRIPES: f32 = 0.35;
const MARKER: f32 = 2.0;
const MAX_STRIPES: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Grip {
    Body,
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Bar {
    pub(crate) edit_point: EditPoint,
    pub(crate) kind: TransitionKind,
    pub(crate) rect: Rect,
    pub(crate) cut: f32,
    pub(crate) frozen: FrozenFrames,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DropTarget {
    pub(crate) edit_point: EditPoint,
    pub(crate) at: Option<TransitionAlignment>,
    pub(crate) x: f32,
    pub(crate) row: Row,
}

fn band(row: Row) -> (f32, f32) {
    let bottom = row.bottom() - INSET;
    (bottom - BAR, bottom)
}

pub(crate) fn bars(
    project: &Project,
    rows: impl IntoIterator<Item = Row>,
    scale: &Scale,
) -> Vec<Bar> {
    let first = scale.first();
    let last = scale.last();
    rows.into_iter()
        .flat_map(|row| {
            project
                .transition_spans(row.layer)
                .into_iter()
                .filter(|transition_span| {
                    transition_span.span.end as f64 >= first
                        && transition_span.span.start as f64 <= last
                })
                .map(|transition_span| Bar {
                    edit_point: transition_span.edit_point,
                    kind: transition_span.transition.kind,
                    rect: spanned(scale, row, &transition_span.span),
                    cut: scale.x_of(transition_span.cut as f64),
                    frozen: project.frozen_frames(&transition_span),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub(crate) fn spanned(scale: &Scale, row: Row, span: &Range<i64>) -> Rect {
    let (top, bottom) = band(row);
    let mut left = scale.x_of(span.start as f64);
    let mut right = scale.x_of(span.end as f64);
    if right - left < NARROWEST {
        let middle = (left + right) / 2.0;
        left = middle - NARROWEST / 2.0;
        right = middle + NARROWEST / 2.0;
    }
    Rect::new(left, top, right - left, bottom - top)
}

pub(crate) fn grip_at(bars: &[Bar], local: Point) -> Option<(EditPoint, Grip)> {
    bars.iter().find_map(|bar| {
        let rect = bar.rect;
        let inside = local.y >= rect.min_y()
            && local.y < rect.max_y()
            && local.x >= rect.min_x() - GRIP
            && local.x < rect.max_x() + GRIP;
        if !inside {
            return None;
        }
        let grip = match () {
            () if (local.x - rect.min_x()).abs() <= GRIP => Grip::Start,
            () if (local.x - rect.max_x()).abs() <= GRIP => Grip::End,
            () => Grip::Body,
        };
        Some((bar.edit_point, grip))
    })
}

fn edges(project: &Project, layer: usize) -> Vec<(EditPoint, i64, bool)> {
    let Some(layer_ref) = project.layer(layer) else {
        return Vec::new();
    };
    let clips = layer_ref.clips();
    clips
        .iter()
        .enumerate()
        .filter(|(_, clip)| project.is_visual(clip))
        .flat_map(|(at, clip)| {
            let joined = |other: Option<&Clip>, after: bool| {
                other.is_some_and(|other| match after {
                    true => project.joins(clip, other),
                    false => project.joins(other, clip),
                })
            };
            let next = clips.get(at + 1).filter(|next| joined(Some(next), true));
            let previous = at.checked_sub(1).and_then(|before| clips.get(before));
            let tail = next.map_or(EditPoint::tail(clip.id()), |next| {
                EditPoint::head(next.id())
            });
            [
                (
                    EditPoint::head(clip.id()),
                    clip.start(),
                    joined(previous, false),
                ),
                (tail, clip.end(), next.is_some()),
            ]
        })
        .collect()
}

pub(crate) fn edit_point_at(
    project: &Project,
    rows: &Rows,
    scale: &Scale,
    local: Point,
) -> Option<EditPoint> {
    let row = rows.at(local.y)?;
    let (top, bottom) = band(row);
    if local.y < top || local.y >= bottom {
        return None;
    }
    edges(project, row.layer)
        .into_iter()
        .map(|(edit_point, frame, _)| (edit_point, (scale.x_of(frame as f64) - local.x).abs()))
        .filter(|(_, gap)| *gap <= REACH)
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(edit_point, _)| edit_point)
}

pub(crate) fn drop_target(
    project: &Project,
    rows: &Rows,
    scale: &Scale,
    local: Point,
) -> Option<DropTarget> {
    let row = rows.at(local.y)?;
    let (edit_point, frame, joined, gap) = edges(project, row.layer)
        .into_iter()
        .map(|(edit_point, frame, joined)| {
            (
                edit_point,
                frame,
                joined,
                local.x - scale.x_of(frame as f64),
            )
        })
        .filter(|edge| edge.3.abs() <= LANDING)
        .min_by(|left, right| left.3.abs().total_cmp(&right.3.abs()))?;
    let at = match (joined, gap) {
        (false, _) => None,
        (true, gap) if gap.abs() <= CENTRE => Some(TransitionAlignment::Centre),
        (true, gap) if gap < 0.0 => Some(TransitionAlignment::End),
        (true, _) => Some(TransitionAlignment::Start),
    };
    Some(DropTarget {
        edit_point,
        at,
        x: scale.x_of(frame as f64),
        row,
    })
}

pub(crate) fn paint_bars(
    ctx: &mut PaintCtx<'_>,
    origin: Point,
    bars: &[Bar],
    selected: Option<EditPoint>,
    per_frame: f32,
) {
    let palette = palette();
    for bar in bars {
        let rect = bar.rect.translate(origin);
        let picked = selected == Some(bar.edit_point);
        let edge = match picked {
            true => palette.accent,
            false => palette.text.scale_alpha(0.6),
        };
        ctx.draw_quad(
            Quad::new(rect)
                .fill(palette.surface.scale_alpha(FILL))
                .stroke(if picked { 2.0 } else { 1.0 }, edge)
                .radius(Corner::Detail),
        );
        ctx.clipped(rect, |ctx| {
            let cut = origin.x + bar.cut;
            let frozen_out = bar.frozen.out as f32 * per_frame;
            let frozen_into = bar.frozen.into as f32 * per_frame;
            let striped = [
                (rect.max_x() - frozen_out, rect.max_x()),
                (rect.min_x(), rect.min_x() + frozen_into),
            ];
            for (from, to) in striped.into_iter().filter(|(from, to)| to > from) {
                paint_stripes(
                    ctx,
                    Rect::new(from, rect.y(), to - from, rect.height()),
                    palette.danger,
                );
            }
            if cut > rect.min_x() && cut < rect.max_x() {
                ctx.draw_quad(
                    Quad::new(Rect::new(cut - 0.5, rect.y(), 1.0, rect.height()))
                        .fill(palette.text.scale_alpha(0.5)),
                );
            }
            let name = wording::transition_kind(bar.kind).to_string();
            let run = TextRun::new(name, NAME).no_wrap().ellipsized();
            ctx.draw_text(rect.inflate(-2.0), run, palette.text);
        });
    }
}

fn paint_stripes(ctx: &mut PaintCtx<'_>, rect: Rect, ink: Color) {
    let count = ((rect.width() + rect.height()) / (STRIPE * 2.0)).ceil() as usize;
    for step in 0..count.min(MAX_STRIPES) {
        let x = rect.min_x() - rect.height() + step as f32 * STRIPE * 2.0;
        ctx.draw_quad(
            Quad::new(Rect::new(x, rect.y(), STRIPE, rect.height())).fill(ink.scale_alpha(STRIPES)),
        );
    }
}

pub(crate) fn paint_edit_point(
    ctx: &mut PaintCtx<'_>,
    origin: Point,
    project: &Project,
    rows: &Rows,
    scale: &Scale,
    edit_point: EditPoint,
) {
    let Some((layer, clip)) = project.locate(edit_point.clip) else {
        return;
    };
    let frame = match edit_point.edge {
        Edge::Head => clip.start(),
        Edge::Tail => clip.end(),
    };
    let row = rows.layer_row(layer);
    let x = origin.x + scale.x_of(frame as f64);
    ctx.draw_quad(
        Quad::new(Rect::new(
            x - MARKER / 2.0,
            origin.y + row.top,
            MARKER,
            row.height,
        ))
        .fill(palette().accent),
    );
}

pub(crate) fn paint_landing(ctx: &mut PaintCtx<'_>, origin: Point, drop_target: DropTarget) {
    let palette = palette();
    let (top, bottom) = band(drop_target.row);
    let x = origin.x + drop_target.x;
    let wide = LANDING;
    let (left, right) = match drop_target.at {
        Some(TransitionAlignment::End) => (x - wide, x),
        Some(TransitionAlignment::Start) => (x, x + wide),
        _ => (x - wide / 2.0, x + wide / 2.0),
    };
    ctx.draw_quad(
        Quad::new(Rect::new(left, origin.y + top, right - left, bottom - top))
            .fill(palette.accent.scale_alpha(0.35))
            .stroke(1.5, palette.accent)
            .radius(Corner::Detail),
    );
    ctx.draw_quad(
        Quad::new(Rect::new(
            x - MARKER / 2.0,
            origin.y + drop_target.row.top,
            MARKER,
            drop_target.row.height,
        ))
        .fill(palette.accent),
    );
}
