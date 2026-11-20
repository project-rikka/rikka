use std::collections::HashMap;

use crate::ids::SplitGroupId;

const PANES: std::ops::RangeInclusive<usize> = 2..=4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

impl SplitAxis {
    pub(crate) fn code(self) -> i64 {
        match self {
            Self::Horizontal => 0,
            Self::Vertical => 1,
        }
    }

    pub(crate) fn from_code(code: i64) -> Self {
        match code {
            1 => Self::Vertical,
            _ => Self::Horizontal,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionSplit {
    pub group: SplitGroupId,
    pub axis: SplitAxis,
    pub shares: Vec<f32>,
}

pub(crate) fn validate(
    splits: &[SessionSplit],
    members: &HashMap<SplitGroupId, usize>,
) -> Vec<SessionSplit> {
    let mut valid: Vec<SessionSplit> = Vec::with_capacity(splits.len());
    for split in splits {
        if valid.iter().any(|accepted| accepted.group == split.group) {
            continue;
        }
        let panes = split.shares.len();
        if !PANES.contains(&panes) || members.get(&split.group) != Some(&panes) {
            continue;
        }
        if !split
            .shares
            .iter()
            .all(|share| share.is_finite() && *share > 0.0)
        {
            continue;
        }
        let total: f32 = split.shares.iter().sum();
        if !total.is_finite() {
            continue;
        }
        valid.push(SessionSplit {
            group: split.group,
            axis: split.axis,
            shares: split.shares.iter().map(|share| share / total).collect(),
        });
    }
    valid
}

pub(crate) fn encode(shares: &[f32]) -> Vec<u8> {
    shares
        .iter()
        .flat_map(|share| share.to_le_bytes())
        .collect()
}

pub(crate) fn decode(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect()
}
