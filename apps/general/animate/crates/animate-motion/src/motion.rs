use std::cmp::Ordering;
use std::ops::Range;

use animate_track::Tracks;

use crate::intensity::Intensity;
use crate::part::{Part, PartKey};

const REACH: usize = 2;

#[derive(Clone, Debug, PartialEq)]
pub struct Automation {
    pub secondary: f32,
    pub blink: bool,
    pub limits: bool,
    pub seed: u64,
    pub fitting: bool,
    pub jiggle: f32,
    pub intensity: Intensity,
}

impl Default for Automation {
    fn default() -> Self {
        Self {
            secondary: 0.3,
            blink: true,
            limits: true,
            seed: 0,
            fitting: true,
            jiggle: 0.5,
            intensity: Intensity::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HumanMotion {
    parts: Tracks<PartKey>,
    pub settings: Automation,
}

impl HumanMotion {
    #[must_use]
    pub fn parts(&self) -> &Tracks<PartKey> {
        &self.parts
    }

    pub fn insert(&mut self, key: PartKey) -> Option<PartKey> {
        assert!(
            key.fits(),
            "a {:?} key must hold that part's values and slopes",
            key.part
        );
        self.parts.insert(key)
    }

    pub fn remove(&mut self, part: Part, frame: u32) -> Option<PartKey> {
        self.parts.remove(&part, frame)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    #[must_use]
    pub fn last_frame(&self) -> u32 {
        self.parts.last_frame().unwrap_or(0)
    }

    fn touched(&self, part: Part, frame: u32) -> Range<u32> {
        let track = self.parts.keys(&part);
        let before = track.partition_point(|key| key.frame < frame);
        let after = track.partition_point(|key| key.frame <= frame);
        let start = before.checked_sub(REACH).map_or(0, |at| track[at].frame);
        let end = track
            .get(after + REACH - 1)
            .map_or(u32::MAX, |key| key.frame + 1);
        start..end
    }

    #[must_use]
    pub fn dirty_since(&self, before: &Self) -> Option<Range<u32>> {
        if !self.settings.evaluates_like(&before.settings) {
            return Some(0..u32::MAX);
        }
        let mut dirty: Option<Range<u32>> = None;
        for part in Part::ALL {
            for frame in changed_frames(self.parts.keys(&part), before.parts.keys(&part)) {
                for span in [self.touched(part, frame), before.touched(part, frame)] {
                    dirty = Some(match dirty {
                        Some(range) => range.start.min(span.start)..range.end.max(span.end),
                        None => span,
                    });
                }
            }
        }
        dirty
    }
}

impl Automation {
    fn evaluates_like(&self, other: &Self) -> bool {
        let limits_matter = !self.intensity.is_neutral() || !other.intensity.is_neutral();
        Self {
            limits: match limits_matter {
                true => self.limits,
                false => other.limits,
            },
            ..self.clone()
        } == *other
    }
}

fn changed_frames(now: &[PartKey], then: &[PartKey]) -> Vec<u32> {
    let mut changed = Vec::new();
    if std::ptr::eq(now, then) {
        return changed;
    }
    let (mut one, mut two) = (now.iter().peekable(), then.iter().peekable());
    loop {
        match (one.peek(), two.peek()) {
            (None, None) => return changed,
            (Some(key), None) => {
                changed.push(key.frame);
                one.next();
            }
            (None, Some(key)) => {
                changed.push(key.frame);
                two.next();
            }
            (Some(new), Some(old)) => match new.frame.cmp(&old.frame) {
                Ordering::Less => {
                    changed.push(new.frame);
                    one.next();
                }
                Ordering::Greater => {
                    changed.push(old.frame);
                    two.next();
                }
                Ordering::Equal => {
                    if new != old {
                        changed.push(new.frame);
                    }
                    one.next();
                    two.next();
                }
            },
        }
    }
}
