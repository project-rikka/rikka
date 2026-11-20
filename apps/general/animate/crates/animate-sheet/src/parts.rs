use animate_motion::{Part, PartKey, Tag};
use animate_track::Tracks;

use crate::sheet::{Group, GroupKind, Lane, Track};

pub(crate) fn group(parts: &Tracks<PartKey>) -> Group {
    let tracks = Part::ALL
        .into_iter()
        .map(|part| {
            let (keys, tags) = keyed(parts, &part);
            Track {
                lane: Lane::Part(part),
                keys,
                tags,
            }
        })
        .collect();
    Group::new(GroupKind::Parts, tracks)
}

pub(crate) fn keyed(parts: &Tracks<PartKey>, part: &Part) -> (Vec<u32>, Vec<Tag>) {
    let keys = parts.keys(part);
    (
        keys.iter().map(|key| key.frame).collect(),
        keys.iter().map(|key| key.tag).collect(),
    )
}
