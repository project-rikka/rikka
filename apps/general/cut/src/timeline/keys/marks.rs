use std::collections::BTreeMap;

use cut_project::{Clip, KeyRef, Property};
use rikka_app::prelude::*;

use rikka_timeline::KeyGlyph;

use super::glyph;
use super::outline::{LANE, Lane, Outline, SUMMARY};
use crate::timeline::scale::Scale;

const REACH: f32 = 7.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Mark {
    pub(crate) frame: i64,
    pub(crate) keys: Vec<KeyRef>,
    pub(crate) glyph: KeyGlyph,
}

pub(crate) fn property_marks(clip: &Clip, property: &Property) -> Vec<Mark> {
    let Some(channel) = clip.channel(property) else {
        return Vec::new();
    };
    channel
        .frames()
        .into_iter()
        .map(|local| {
            let frame = clip.start() + local;
            Mark {
                frame,
                keys: vec![KeyRef::new(property.clone(), frame)],
                glyph: channel
                    .timing(local)
                    .map_or(glyph::SUMMARY, glyph::of_timing),
            }
        })
        .collect()
}

pub(crate) fn summary(clip: &Clip, outline: &Outline) -> Vec<Mark> {
    let mut merged: BTreeMap<i64, Vec<KeyRef>> = BTreeMap::new();
    for property in &outline.properties {
        for mark in property_marks(clip, property) {
            merged.entry(mark.frame).or_default().extend(mark.keys);
        }
    }
    merged
        .into_iter()
        .map(|(frame, keys)| Mark {
            frame,
            keys,
            glyph: glyph::SUMMARY,
        })
        .collect()
}

pub(crate) fn in_lane(clip: &Clip, outline: &Outline, lane: Lane) -> Vec<Mark> {
    match lane {
        Lane::Summary => summary(clip, outline),
        Lane::Property(index) => outline
            .properties
            .get(index)
            .map(|property| property_marks(clip, property))
            .unwrap_or_default(),
        Lane::Graph => Vec::new(),
    }
}

pub(crate) fn nearest(marks: &[Mark], scale: &Scale, x: f32) -> Option<Mark> {
    scale
        .shown()
        .nearest(marks, |mark| mark.frame, x, REACH)
        .cloned()
}

pub(crate) fn banded(clip: &Clip, outline: &Outline, scale: &Scale, band: Rect) -> Vec<KeyRef> {
    let lanes = std::iter::once((Lane::Summary, 0.0, SUMMARY)).chain(
        (0..outline.properties.len())
            .filter(|_| !outline.graph)
            .map(|index| (Lane::Property(index), Outline::lane_top(index), LANE)),
    );
    lanes
        .filter(|(_, top, height)| *top < band.max_y() && top + height > band.min_y())
        .flat_map(|(lane, _, _)| in_lane(clip, outline, lane))
        .filter(|mark| {
            let x = scale.x_of(mark.frame as f64);
            x >= band.min_x() && x <= band.max_x()
        })
        .flat_map(|mark| mark.keys)
        .collect()
}
