use animate_body::Faces;
use animate_motion::{HumanMotion, Tag, Take};
use animate_track::Tracks;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::display::{BoneDisplay, BoneGroup};
use crate::keys;
use crate::morphs;
use crate::parts;
use crate::sheet::{Group, GroupKind, Lane, Sheet, Track};

#[derive(Clone, Copy)]
pub enum Listing<'a> {
    Parts,
    Bones {
        display: &'a BoneDisplay,
        shown: &'a dyn Fn(&str) -> bool,
    },
    Morphs,
}

impl std::fmt::Debug for Listing<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parts => f.write_str("Parts"),
            Self::Bones { .. } => f.debug_struct("Bones").finish_non_exhaustive(),
            Self::Morphs => f.write_str("Morphs"),
        }
    }
}

impl Sheet {
    #[must_use]
    pub fn of_take(listing: Listing<'_>, faces: &Faces, take: &Take) -> Self {
        let mut groups = match listing {
            Listing::Parts => {
                let empty = Tracks::default();
                vec![parts::group(
                    take.human.as_ref().map_or(&empty, HumanMotion::parts),
                )]
            }
            Listing::Bones { display, shown } => bones(display, take, shown),
            Listing::Morphs => Vec::new(),
        };
        morphs::place(&mut groups, faces, &take.morphs);
        let mut sheet = Sheet::new(groups);
        if let Listing::Parts = listing {
            sheet.rehold(&take.holds);
        }
        sheet
    }

    #[must_use]
    pub fn retaken(&self, listing: Listing<'_>, faces: &Faces, before: &Take, take: &Take) -> Self {
        let mut sheet = self.clone();
        let patched = match listing {
            Listing::Parts => sheet.repart(before, take),
            Listing::Bones { shown, .. } => sheet.rebone(before, take, shown),
            Listing::Morphs => true,
        } && sheet.remorph(faces, before, take);
        match patched {
            true => sheet,
            false => Self::of_take(listing, faces, take),
        }
    }

    fn repart(&mut self, before: &Take, take: &Take) -> bool {
        let empty = Tracks::default();
        let was = before.human.as_ref().map_or(&empty, HumanMotion::parts);
        let now = take.human.as_ref().map_or(&empty, HumanMotion::parts);
        for part in now.touched(was) {
            let (keys, tags) = parts::keyed(now, part);
            self.patch(&Lane::Part(*part), keys, tags, |_| true);
        }
        if before.holds != take.holds {
            self.rehold(&take.holds);
        }
        true
    }

    fn rebone(&mut self, before: &Take, take: &Take, shown: &dyn Fn(&str) -> bool) -> bool {
        if take.switches.touched(&before.switches).next().is_some() {
            let Some(at) = self
                .groups
                .iter()
                .position(|group| group.kind == GroupKind::Switches)
            else {
                return false;
            };
            self.rekey(
                at,
                keys::sorted(take.switches.all_keys().map(|key| key.frame)),
            );
        }
        take.bones
            .touched(&before.bones)
            .filter(|name| shown(name))
            .all(|name| {
                let keys = take.bones.keys(name).iter().map(|key| key.frame).collect();
                self.patch(&Lane::Bone(name.clone()), keys, Vec::new(), |kind| {
                    matches!(kind, GroupKind::Listed(_) | GroupKind::Region(_))
                })
            })
    }

    fn remorph(&mut self, faces: &Faces, before: &Take, take: &Take) -> bool {
        take.morphs.touched(&before.morphs).all(|name| {
            let listed = morphs::always_listed(faces, name);
            self.patch(
                &Lane::Morph(name.clone()),
                morphs::keys(&take.morphs, name),
                Vec::new(),
                |kind| listed && matches!(kind, GroupKind::Morphs(_)),
            )
        })
    }

    fn patch(
        &mut self,
        lane: &Lane,
        keys: Vec<u32>,
        tags: Vec<Tag>,
        listed: impl Fn(&GroupKind) -> bool,
    ) -> bool {
        let rows: Vec<(usize, usize)> = self
            .groups
            .iter()
            .enumerate()
            .flat_map(|(at, group)| {
                group
                    .tracks
                    .iter()
                    .enumerate()
                    .filter(|(_, track)| track.lane == *lane)
                    .map(move |(track, _)| (at, track))
            })
            .collect();
        if rows.is_empty() {
            return keys.is_empty();
        }
        if keys.is_empty() && rows.iter().any(|(at, _)| !listed(&self.groups[*at].kind)) {
            return false;
        }
        for (group, track) in rows {
            self.retrack(group, track, keys.clone(), tags.clone());
        }
        true
    }
}

fn bones(display: &BoneDisplay, take: &Take, shown: &dyn Fn(&str) -> bool) -> Vec<Group> {
    let bones: FxHashMap<&str, Vec<u32>> = take
        .bones
        .tracks()
        .filter(|(name, _)| shown(name))
        .map(|(name, keyed)| (name.as_str(), keyed.iter().map(|key| key.frame).collect()))
        .collect();
    let bone = |name: &str| {
        Track::plain(
            Lane::Bone(name.to_owned()),
            bones.get(name).cloned().unwrap_or_default(),
        )
    };

    let mut groups = Vec::new();
    if display.has_switches() {
        groups.push(Group::keyed(
            GroupKind::Switches,
            None,
            keys::sorted(take.switches.all_keys().map(|key| key.frame)),
        ));
    }

    let mut displayed = FxHashSet::default();
    for (group, members) in display.groups() {
        let tracks: Vec<Track> = members
            .iter()
            .filter(|name| shown(name))
            .map(|name| {
                displayed.insert(name.as_str());
                bone(name)
            })
            .collect();
        if !tracks.is_empty() {
            let kind = match group {
                BoneGroup::Frame(name) => GroupKind::Listed(name.clone()),
                BoneGroup::Region(part) => GroupKind::Region(*part),
            };
            groups.push(Group::new(kind, tracks));
        }
    }

    let unlisted: Vec<Track> = display
        .bones()
        .iter()
        .map(String::as_str)
        .filter(|name| !displayed.contains(name) && bones.contains_key(name))
        .map(bone)
        .collect();
    if !unlisted.is_empty() {
        groups.push(Group::new(GroupKind::Unlisted, unlisted));
    }

    let bone_names: FxHashSet<&str> = display.bones().iter().map(String::as_str).collect();
    let mut unmatched: Vec<Track> = bones
        .keys()
        .filter(|name| !bone_names.contains(*name))
        .map(|name| bone(name))
        .collect();
    unmatched.sort_by(|one, two| one.lane.cmp(&two.lane));
    if !unmatched.is_empty() {
        groups.push(Group::new(GroupKind::Unmatched, unmatched));
    }
    groups
}
