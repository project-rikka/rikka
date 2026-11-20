use animate_edit::KeyId;
use rikka_app::prelude::*;
use rustc_hash::FxHashSet;

use crate::layout::{Axis, Geometry, Row};
use crate::state::Board;

const KEY_REACH: f32 = 5.0;

pub(crate) fn reading(axis: Axis, point: Point) -> f32 {
    match axis {
        Axis::Time => point.x,
        Axis::Rows => point.y,
    }
}

pub(crate) fn lane_at(geometry: &Geometry, position: Point) -> Option<Axis> {
    [Axis::Time, Axis::Rows]
        .into_iter()
        .find(|axis| geometry.scrollbar_track(*axis).contains(position))
}

pub(crate) fn row_keys(board: &Board, row: Row) -> &[u32] {
    match row {
        Row::Sound | Row::Pour | Row::Hold(_) => &[],
        Row::Group(group) => &board.sheet.groups()[group].keys,
        Row::Track(group, track) => &board.sheet.groups()[group].tracks[track].keys,
    }
}

pub(crate) fn ids_at(board: &Board, row: Row, frame: u32) -> FxHashSet<KeyId> {
    match row {
        Row::Sound | Row::Pour | Row::Hold(_) => FxHashSet::default(),
        Row::Group(group) => board.sheet.groups()[group]
            .lanes_keyed_at(frame)
            .into_iter()
            .map(|lane| KeyId::new(lane.clone(), frame))
            .collect(),
        Row::Track(group, track) => {
            let lane = &board.sheet.groups()[group].tracks[track].lane;
            FxHashSet::from_iter([KeyId::new(lane.clone(), frame)])
        }
    }
}

pub(crate) fn key_near(geometry: &Geometry, keys: &[u32], x: f32) -> Option<u32> {
    geometry
        .lane()
        .nearest(keys, |key| *key, x, KEY_REACH)
        .copied()
}
