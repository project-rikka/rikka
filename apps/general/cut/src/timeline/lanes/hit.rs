use cut_project::{Clip, ClipId, Edge, EditPoint, MarkerId, Project};
use rikka_app::prelude::*;

use super::parts::{self, Parts, fade_handle, level_keys, level_y};
use super::transition_spans::{self, Grip};
use super::wedges::{self, WedgeSide};
use crate::timeline::rows::{Row, Rows};
use crate::timeline::scale::Scale;

const EDGE: f32 = 6.0;
const NARROWEST: f32 = 3.0 * EDGE;
const DISCLOSURE_PAD: f32 = 4.0;
const LEVEL_REACH: f32 = 4.0;
const KEY_REACH: f32 = 5.0;
const MARKER_REACH: f32 = 5.0;
pub(crate) const DISCLOSURE: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    Body,
    End(Edge),
    AnimationPreset(WedgeSide),
    TransitionSpan(EditPoint, Grip),
    EditPoint(EditPoint),
    Fade(Edge),
    Level,
    LevelKey(i64),
    Marker(MarkerId),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Hit {
    pub(crate) clip: ClipId,
    pub(crate) part: Part,
    pub(crate) locked: bool,
    pub(crate) frame: i64,
}

pub(crate) fn span(scale: &Scale, clip: &Clip) -> (f32, f32) {
    (
        scale.x_of(clip.start() as f64),
        scale.x_of(clip.end() as f64),
    )
}

pub(crate) fn disclosure(from: f32, to: f32) -> Option<(f32, f32)> {
    let left = from.max(0.0) + DISCLOSURE_PAD;
    (to - left >= DISCLOSURE + DISCLOSURE_PAD).then_some((left, left + DISCLOSURE))
}

pub(crate) fn discloses(
    project: &Project,
    rows: &Rows,
    scale: &Scale,
    clip: ClipId,
    local: Point,
) -> bool {
    let Some((layer, clip_ref)) = project.locate(clip) else {
        return false;
    };
    let row = rows.layer_row(layer);
    let (from, to) = span(scale, clip_ref);
    local.y >= row.top
        && local.y < row.bottom()
        && disclosure(from, to).is_some_and(|(left, right)| local.x >= left && local.x < right)
}

pub(crate) fn parts_of(project: &Project, scale: &Scale, row: &Row, clip: &Clip) -> Parts {
    let media = clip.media().and_then(|media| project.media(media));
    Parts::layout(parts::body(scale, row, clip.start(), clip.end()), media)
}

pub(crate) fn hit(project: &Project, rows: &Rows, scale: &Scale, local: Point) -> Option<Hit> {
    let row = rows.at(local.y)?;
    let layer = project.layer(row.layer)?;
    let locked = layer.is_locked();
    let frame = scale.frame_at(local.x).round().max(0.0) as i64;
    let seamed = |edit_point: EditPoint, part: Part| Hit {
        clip: edit_point.clip,
        part,
        locked,
        frame,
    };
    let bars = transition_spans::bars(project, [row], scale);
    if let Some((edit_point, grip)) = transition_spans::grip_at(&bars, local) {
        return Some(seamed(edit_point, Part::TransitionSpan(edit_point, grip)));
    }
    if let Some(edit_point) = transition_spans::edit_point_at(project, rows, scale, local) {
        return Some(seamed(edit_point, Part::EditPoint(edit_point)));
    }
    let under = scale.frame_at(local.x).floor() as i64;
    let reach = scale.frames_across(EDGE).ceil() as i64 + 1;
    let clip = layer
        .clips_in(under - reach..under + reach + 1)
        .iter()
        .find(|clip| {
            let (from, to) = span(scale, clip);
            local.x >= from && local.x < to
        })?;
    let parts = parts_of(project, scale, &row, clip);
    let (from, to) = span(scale, clip);
    let wide = to - from >= NARROWEST;
    let edge = match wide {
        true if local.x - from <= EDGE => Some(Edge::Head),
        true if to - local.x <= EDGE => Some(Edge::Tail),
        _ => None,
    };
    let part = sounded(scale, clip, &parts, local)
        .filter(|part| matches!(part, Part::Fade(_)) || edge.is_none())
        .or_else(|| edge.map(Part::End))
        .or_else(|| marker_at(scale, clip, &parts, local).map(Part::Marker))
        .or_else(|| wedges::edge_at(clip, scale, local.x).map(Part::AnimationPreset))
        .unwrap_or(Part::Body);
    Some(Hit {
        clip: clip.id(),
        part,
        locked,
        frame,
    })
}

fn sounded(scale: &Scale, clip: &Clip, parts: &Parts, local: Point) -> Option<Part> {
    let sound = parts.sound?;
    let mix = clip.treatment()?.mix;
    for edge in [Edge::Head, Edge::Tail] {
        if fade_handle(scale, clip, sound, mix, edge)
            .inflate(1.0)
            .contains(local)
        {
            return Some(Part::Fade(edge));
        }
    }
    if !sound.contains(local) {
        return None;
    }
    let key = level_keys(scale, clip, sound).find(|(_, spot)| {
        (spot.x - local.x).abs() <= KEY_REACH && (spot.y - local.y).abs() <= KEY_REACH
    });
    if let Some((frame, _)) = key {
        return Some(Part::LevelKey(frame));
    }
    let frame = scale.frame_at(local.x);
    let volume = clip
        .audible_mix(frame, None)
        .map_or(mix.volume, |preview| preview.volume);
    ((level_y(sound, volume) - local.y).abs() <= LEVEL_REACH).then_some(Part::Level)
}

fn marker_at(scale: &Scale, clip: &Clip, parts: &Parts, local: Point) -> Option<MarkerId> {
    if !parts.header.contains(local) {
        return None;
    }
    clip.markers()
        .iter()
        .filter_map(|(id, marker)| Some((id, clip.marker_span(marker)?)))
        .map(|(id, span)| (id, (scale.x_of(span.start as f64) - local.x).abs()))
        .filter(|(_, distance)| *distance <= MARKER_REACH)
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(id, _)| id)
}

pub(crate) fn banded(project: &Project, rows: &Rows, scale: &Scale, band: Rect) -> Vec<ClipId> {
    let first = scale.frame_at(band.min_x()).floor() as i64;
    let last = scale.frame_at(band.max_x()).ceil() as i64;
    rows.all()
        .filter(|row| row.top < band.max_y() && row.bottom() > band.min_y())
        .filter_map(|row| project.layer(row.layer))
        .filter(|layer| !layer.is_locked())
        .flat_map(|layer| layer.clips_in(first..last.max(first + 1)))
        .map(Clip::id)
        .collect()
}
