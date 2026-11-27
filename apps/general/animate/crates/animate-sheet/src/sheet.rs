use std::sync::Arc;

use animate_body::Panel;
use animate_fluid::{EmitterSection, EmitterSetting, Pour};
use animate_look::{LookGroup, Target};
use animate_motion::{Part, Tag};

use crate::holding::HoldRow;
use crate::keys::{self, Tally};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Lane {
    Bone(String),
    Morph(String),
    Camera,
    Part(Part),
    Look(Target),
    Fluid(EmitterSetting),
    Wind(animate_wind::WindSetting),
    Prop(animate_prop::PropSetting),
    Anchor,
}

impl Lane {
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Bone(name) | Self::Morph(name) => name,
            Self::Camera => "",
            Self::Part(part) => part.name(),
            Self::Look(target) => target.parameter(),
            Self::Fluid(setting) => setting.id(),
            Self::Wind(setting) => setting.id(),
            Self::Prop(setting) => setting.id(),
            Self::Anchor => "anchor",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroupKind {
    Switches,
    Listed(String),
    Region(Part),
    Unlisted,
    Unmatched,
    Camera,
    Parts,
    Morphs(Panel),
    Look {
        group: LookGroup,
        title: Option<String>,
    },
    Fluid(EmitterSection),
    Wind,
    Prop,
}

impl GroupKind {
    #[must_use]
    pub fn stays_open(&self) -> bool {
        matches!(self, Self::Parts | Self::Fluid(_) | Self::Wind | Self::Prop)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Track {
    pub lane: Lane,
    pub keys: Vec<u32>,
    pub tags: Vec<Tag>,
}

impl Track {
    pub(crate) fn plain(lane: Lane, keys: Vec<u32>) -> Self {
        Self {
            lane,
            keys,
            tags: Vec::new(),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.lane.name()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub kind: GroupKind,
    pub lane: Option<Lane>,
    pub tracks: Vec<Track>,
    pub keys: Vec<u32>,
}

impl Group {
    pub(crate) fn new(kind: GroupKind, tracks: Vec<Track>) -> Self {
        let keys = keys::sorted(tracks.iter().flat_map(|track| track.keys.iter().copied()));
        Self {
            kind,
            lane: None,
            tracks,
            keys,
        }
    }

    pub(crate) fn keyed(kind: GroupKind, lane: Option<Lane>, keys: Vec<u32>) -> Self {
        Self {
            kind,
            lane,
            tracks: Vec::new(),
            keys,
        }
    }

    #[must_use]
    pub fn opens(&self) -> bool {
        !self.tracks.is_empty()
    }

    pub fn lanes(&self) -> impl Iterator<Item = &Lane> {
        self.lane
            .iter()
            .chain(self.tracks.iter().map(|track| &track.lane))
    }

    #[must_use]
    pub fn lanes_keyed_at(&self, frame: u32) -> Vec<&Lane> {
        self.lane
            .iter()
            .filter(|_| self.keys.binary_search(&frame).is_ok())
            .chain(
                self.tracks
                    .iter()
                    .filter(|track| track.keys.binary_search(&frame).is_ok())
                    .map(|track| &track.lane),
            )
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pouring {
    pub pour: Pour,
    pub flows: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sheet {
    pub(crate) groups: Vec<Arc<Group>>,
    pub(crate) tally: Tally,
    pouring: Option<Pouring>,
    pub(crate) holding: Vec<HoldRow>,
    pub(crate) marks: Vec<u32>,
}

impl Sheet {
    pub(crate) fn new(groups: Vec<Group>) -> Self {
        let groups: Vec<Arc<Group>> = groups.into_iter().map(Arc::new).collect();
        Self {
            tally: Tally::of(groups.iter().map(|group| group.keys.as_slice())),
            groups,
            pouring: None,
            holding: Vec::new(),
            marks: Vec::new(),
        }
    }

    pub(crate) fn with_pouring(mut self, pouring: Pouring) -> Self {
        self.pouring = Some(pouring);
        self
    }

    pub(crate) fn retrack(&mut self, group: usize, track: usize, keys: Vec<u32>, tags: Vec<Tag>) {
        let entry = Arc::make_mut(&mut self.groups[group]);
        entry.tracks[track].keys = keys;
        entry.tracks[track].tags = tags;
        let union = keys::sorted(
            entry
                .tracks
                .iter()
                .flat_map(|track| track.keys.iter().copied()),
        );
        self.rekey(group, union);
    }

    pub(crate) fn rekey(&mut self, group: usize, keys: Vec<u32>) {
        let entry = Arc::make_mut(&mut self.groups[group]);
        let before = std::mem::replace(&mut entry.keys, keys);
        self.tally = self.tally.swapped(&before, &entry.keys);
    }

    pub(crate) fn remark(&mut self, marks: Vec<u32>) {
        self.tally = self.tally.swapped(&self.marks, &marks);
        self.marks = marks;
    }

    #[must_use]
    pub fn pouring(&self) -> Option<Pouring> {
        self.pouring
    }

    #[must_use]
    pub fn groups(&self) -> &[Arc<Group>] {
        &self.groups
    }

    #[must_use]
    pub fn keys(&self) -> &[u32] {
        self.tally.frames()
    }

    #[must_use]
    pub fn last_key(&self) -> Option<u32> {
        self.keys().last().copied()
    }

    #[must_use]
    pub fn next_key(&self, after: u32) -> Option<u32> {
        let keys = self.keys();
        keys.get(keys.partition_point(|key| *key <= after)).copied()
    }

    #[must_use]
    pub fn previous_key(&self, before: u32) -> Option<u32> {
        let keys = self.keys();
        let next = keys.partition_point(|key| *key < before);
        next.checked_sub(1).map(|at| keys[at])
    }
}
