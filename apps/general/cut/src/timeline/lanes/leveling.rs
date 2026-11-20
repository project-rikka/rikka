use cut_project::{
    Appearance, Clip, ClipId, Edge, Edit, KeyChange, KeyRef, KeysChange, MixField, Property,
    Redraw, Treatment, Value,
};
use rikka_app::prelude::*;

use super::grab::frames;
use super::parts::{level_y, lowered, raised, volume_at};
use crate::timeline::scale::Scale;

const VOLUME: Property = Property::Mix(MixField::Volume);

#[derive(Clone, Debug, PartialEq)]
enum Original {
    Still(Treatment),
    DiamondTarget(Vec<(i64, f32)>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Leveling {
    at: Point,
    pub(crate) clip: ClipId,
    start: i64,
    sound: Rect,
    original: Original,
    lift: f32,
}

impl Leveling {
    pub(crate) fn begin(at: Point, clip: &Clip, sound: Rect, frame: i64) -> Option<Leveling> {
        let original = match clip.channel(&VOLUME) {
            Some(channel) => {
                let local = clip.local(frame);
                let frames = channel.frames();
                let before = frames.iter().rev().find(|at| **at <= local).copied();
                let after = frames.iter().find(|at| **at > local).copied();
                let keyed: Vec<(i64, f32)> = before
                    .into_iter()
                    .chain(after)
                    .filter_map(|at| {
                        let value = channel.value(at)?.scalar()?;
                        Some((clip.start() + at, value))
                    })
                    .collect();
                Original::DiamondTarget(keyed)
            }
            None => Original::Still(clip.treatment_at(clip.start())?),
        };
        Some(Leveling {
            at,
            clip: clip.id(),
            start: clip.start(),
            sound,
            original,
            lift: 0.0,
        })
    }

    pub(crate) fn drag(&mut self, to: Point) {
        let reach = self.sound.height().max(f32::EPSILON);
        self.lift = (self.at.y - to.y) / reach;
    }

    pub(crate) fn has_moved(&self) -> bool {
        self.lift != 0.0
    }

    pub(crate) fn edits(&self) -> Vec<Edit> {
        let lifted = |volume: f32| lowered(raised(volume) + self.lift);
        match &self.original {
            Original::Still(treatment) => {
                let mut treatment = *treatment;
                treatment.mix.volume = lifted(treatment.mix.volume);
                vec![Edit::Redraw {
                    changes: vec![(self.clip, Appearance::Footage(treatment))],
                    by: Redraw::Style,
                    at: self.start,
                }]
            }
            Original::DiamondTarget(keys) => keys
                .iter()
                .map(|(at, value)| Edit::Key {
                    clip: self.clip,
                    property: VOLUME,
                    change: KeyChange::Value {
                        at: *at,
                        value: Value::Scalar(lifted(*value)),
                    },
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Rekeying {
    at: Point,
    pub(crate) clip: ClipId,
    from: i64,
    first: i64,
    last: i64,
    sound: Rect,
    by: i64,
    volume: f32,
    original: f32,
}

impl Rekeying {
    pub(crate) fn begin(at: Point, clip: &Clip, sound: Rect, from: i64) -> Option<Rekeying> {
        let original = clip
            .channel(&VOLUME)?
            .value(from - clip.start())?
            .scalar()?;
        Some(Rekeying {
            at,
            clip: clip.id(),
            from,
            first: clip.start(),
            last: clip.end() - 1,
            sound,
            by: 0,
            volume: original,
            original,
        })
    }

    pub(crate) fn drag(&mut self, to: Point, scale: &Scale) {
        self.by =
            frames(scale, to.x - self.at.x).clamp(self.first - self.from, self.last - self.from);
        let dragged_y = level_y(self.sound, self.original) + (to.y - self.at.y);
        self.volume = volume_at(self.sound, dragged_y);
    }

    pub(crate) fn has_moved(&self) -> bool {
        self.by != 0 || self.volume != self.original
    }

    pub(crate) fn edits(&self) -> Vec<Edit> {
        let mut edits = Vec::with_capacity(2);
        if self.by != 0 {
            edits.push(Edit::Keys {
                clip: self.clip,
                keys: vec![KeyRef::new(VOLUME, self.from)],
                change: KeysChange::Move { by: self.by },
            });
        }
        edits.push(Edit::Key {
            clip: self.clip,
            property: VOLUME,
            change: KeyChange::Value {
                at: self.from + self.by,
                value: Value::Scalar(self.volume),
            },
        });
        edits
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Fading {
    at: Point,
    pub(crate) clip: ClipId,
    edge: Edge,
    start: i64,
    treatment: Treatment,
    original: i64,
    longest: i64,
    length: i64,
}

impl Fading {
    pub(crate) fn begin(at: Point, clip: &Clip, edge: Edge) -> Option<Fading> {
        let treatment = clip.treatment_at(clip.start())?;
        let mix = treatment.mix;
        let (original, other) = match edge {
            Edge::Head => (mix.fade_in.length, mix.fade_out.length),
            Edge::Tail => (mix.fade_out.length, mix.fade_in.length),
        };
        Some(Fading {
            at,
            clip: clip.id(),
            edge,
            start: clip.start(),
            treatment,
            original,
            longest: clip.length() - other,
            length: original,
        })
    }

    pub(crate) fn drag(&mut self, to: Point, scale: &Scale) {
        let moved = frames(scale, to.x - self.at.x);
        let length = match self.edge {
            Edge::Head => self.original + moved,
            Edge::Tail => self.original - moved,
        };
        self.length = length.clamp(0, self.longest.max(0));
    }

    pub(crate) fn has_moved(&self) -> bool {
        self.length != self.original
    }

    pub(crate) fn edit(&self) -> Edit {
        let mut treatment = self.treatment;
        match self.edge {
            Edge::Head => treatment.mix.fade_in.length = self.length,
            Edge::Tail => treatment.mix.fade_out.length = self.length,
        }
        Edit::Redraw {
            changes: vec![(self.clip, Appearance::Footage(treatment))],
            by: Redraw::Style,
            at: self.start,
        }
    }
}
