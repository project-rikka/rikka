use std::cmp::Reverse;

use crate::navigation::toolbar::group::ToolPriority;

const TOLERANCE: f32 = 0.5;

#[derive(Clone, Copy, Debug)]
pub(super) enum Measure {
    Group {
        width: f32,
        priority: ToolPriority,
        overflows: bool,
    },
    Flexible {
        least: f32,
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Room {
    pub(super) inner: f32,
    pub(super) gap: f32,
    pub(super) divider: Option<f32>,
    pub(super) more: f32,
}

pub(super) fn hidden(entries: &[Measure], room: &Room) -> Vec<bool> {
    let mut hidden = vec![false; entries.len()];
    let mut order: Vec<(ToolPriority, usize)> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| match *entry {
            Measure::Group { priority, .. } if priority != ToolPriority::Pinned => {
                Some((priority, index))
            }
            Measure::Group { .. } | Measure::Flexible { .. } => None,
        })
        .collect();
    order.sort_by_key(|&(priority, index)| (priority, Reverse(index)));
    let mut collapsing = order.into_iter();
    while needed(entries, &hidden, room) > room.inner + TOLERANCE {
        let Some((_, index)) = collapsing.next() else {
            break;
        };
        hidden[index] = true;
    }
    hidden
}

pub(super) fn divided(groups: &[bool], hidden: &[bool], divider: bool) -> Vec<bool> {
    let mut after_group = false;
    groups
        .iter()
        .zip(hidden)
        .map(|(&group, &hidden)| {
            if hidden {
                return false;
            }
            let divided = divider && group && after_group;
            after_group = group;
            divided
        })
        .collect()
}

fn overflows(entries: &[Measure], hidden: &[bool]) -> bool {
    entries.iter().zip(hidden).any(|(entry, &hidden)| {
        hidden
            && matches!(
                entry,
                Measure::Group {
                    overflows: true,
                    ..
                }
            )
    })
}

fn needed(entries: &[Measure], hidden: &[bool], room: &Room) -> f32 {
    let groups: Vec<bool> = entries
        .iter()
        .map(|entry| matches!(entry, Measure::Group { .. }))
        .collect();
    let divided = divided(&groups, hidden, room.divider.is_some());
    let mut total = 0.0;
    let mut shown = 0_usize;
    for ((entry, &hidden), &divided) in entries.iter().zip(hidden).zip(&divided) {
        if hidden {
            continue;
        }
        total += match *entry {
            Measure::Group { width, .. } => width,
            Measure::Flexible { least } => least,
        };
        if divided {
            total += room.divider.unwrap_or_default();
        }
        shown += 1;
    }
    if overflows(entries, hidden) {
        total += room.more;
        shown += 1;
    }
    total + room.gap * shown.saturating_sub(1) as f32
}
