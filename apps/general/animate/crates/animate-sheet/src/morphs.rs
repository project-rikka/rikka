use animate_body::{Faces, Panel};
use animate_motion::Morphs;

use crate::sheet::{Group, GroupKind, Lane, Track};

pub(crate) fn place(groups: &mut Vec<Group>, faces: &Faces, morphs: &Morphs) {
    let track = |name: &str| Track::plain(Lane::Morph(name.to_owned()), keys(morphs, name));
    let panels: Vec<Group> = Panel::ALL
        .into_iter()
        .filter_map(|panel| {
            let tracks: Vec<Track> = faces
                .morphs()
                .iter()
                .filter(|morph| match morph.panel() {
                    Some(own) => own == panel,
                    None => panel == Panel::Other && !morphs.keys(morph.name()).is_empty(),
                })
                .map(|morph| track(morph.name()))
                .collect();
            (!tracks.is_empty()).then(|| Group::new(GroupKind::Morphs(panel), tracks))
        })
        .collect();
    let unmatched: Vec<Track> = morphs
        .keyed()
        .filter(|name| faces.morph(name).is_none())
        .map(|name| track(name))
        .collect();
    let at = groups
        .iter()
        .position(|group| group.kind == GroupKind::Unmatched)
        .unwrap_or(groups.len());
    groups.splice(at..at, panels);
    if unmatched.is_empty() {
        return;
    }
    let mut tracks = match groups.last() {
        Some(group) if group.kind == GroupKind::Unmatched => {
            groups.pop().map_or_else(Vec::new, |group| group.tracks)
        }
        _ => Vec::new(),
    };
    tracks.extend(unmatched);
    tracks.sort_by(|one, two| one.lane.cmp(&two.lane));
    groups.push(Group::new(GroupKind::Unmatched, tracks));
}

pub(crate) fn keys(morphs: &Morphs, name: &str) -> Vec<u32> {
    morphs.keys(name).iter().map(|key| key.frame).collect()
}

pub(crate) fn always_listed(faces: &Faces, name: &str) -> bool {
    faces
        .morph(name)
        .is_some_and(|morph| morph.panel().is_some())
}
