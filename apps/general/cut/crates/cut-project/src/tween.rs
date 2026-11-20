use rikka_keyframe::{Ease, Easing, Family};

use crate::animation::Tracked;
use crate::camera::CameraField;
use crate::clip::{Clip, ClipId, Source};
use crate::edit::Applied;
use crate::error::{Error, Result};
use crate::motion::Tweening;
use crate::placement::PlacementField;
use crate::project::Project;
use crate::property::Field;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween {
    pub start: i64,
    pub end: i64,
    pub easing: Easing,
}

impl Tween {
    pub const EASING: Easing = Easing::new(Family::Cubic, Ease::InOut);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TweenChange {
    On,
    Off { at: i64 },
    Ease(Easing),
    Reverse,
}

impl Clip {
    #[must_use]
    pub fn tween(&self) -> Option<Tween> {
        let tweening = self.tweening()?;
        Some(Tween {
            start: self.start,
            end: self.start + tweening.last,
            easing: tweening.easing,
        })
    }

    #[must_use]
    pub fn can_tween(&self) -> bool {
        self.length >= 2
    }

    pub(crate) fn tweening(&self) -> Option<Tweening> {
        self.motion.tweening(self.tween_fields(), self.length - 1)
    }

    pub(crate) fn stretching(&mut self, change: impl FnOnce(&mut Clip)) {
        let tracks = match self.tweening() {
            Some(tweening) => self.motion.take_all(&tweening.fields),
            None => Vec::new(),
        };
        change(self);
        let last = self.length - 1;
        for (field, track) in tracks {
            self.motion.insert(field, track.spanning(last));
        }
    }

    fn tween_fields(&self) -> Vec<Field> {
        match self.source {
            Source::Camera(_) => CameraField::ALL.map(Field::Camera).to_vec(),
            Source::Footage { .. } | Source::Graphic(_) | Source::Region(_) => {
                PlacementField::ALL.map(Field::Placement).to_vec()
            }
        }
    }

    fn begin_tween(&mut self) -> Result<()> {
        if !self.can_tween() {
            return Err(Error::TooBrief);
        }
        if self.tweening().is_some() {
            return Ok(());
        }
        let tweening = Tweening {
            fields: self.tween_fields(),
            last: self.length - 1,
            easing: Tween::EASING,
        };
        let mut began = false;
        for (field, track) in self.motion.take_all(&tweening.fields) {
            let (Some(start), Some(end)) = (track.sample(0.0), track.sample(tweening.last as f64))
            else {
                continue;
            };
            self.motion.insert(field, tweening.track(field, start, end));
            began = true;
        }
        let lead = tweening.fields[0];
        if !began && let Some(base) = self.base(lead) {
            self.motion.insert(lead, tweening.track(lead, base, base));
        }
        Ok(())
    }

    fn end_tween(&mut self, at: i64) -> Result<()> {
        let tweening = self.tweening().ok_or(Error::NotTweened)?;
        let local = self.local(at) as f64;
        for (field, track) in self.motion.take_all(&tweening.fields) {
            if let Some(value) = track.sample(local) {
                self.rebase(field, value);
            }
        }
        Ok(())
    }

    fn retween(&mut self, change: impl Fn(&Tracked) -> Tracked) -> Result<()> {
        let tweening = self.tweening().ok_or(Error::NotTweened)?;
        for (field, track) in self.motion.take_all(&tweening.fields) {
            self.motion.insert(field, change(&track));
        }
        Ok(())
    }
}

impl Project {
    pub(crate) fn tween(&mut self, id: ClipId, change: TweenChange) -> Result<Applied> {
        let clip = self.animated_clip_mut(id)?;
        match change {
            TweenChange::On => clip.begin_tween(),
            TweenChange::Off { at } => clip.end_tween(at),
            TweenChange::Ease(easing) => clip.retween(|track| track.eased(easing)),
            TweenChange::Reverse => clip.retween(Tracked::reversed),
        }?;
        Ok(Applied::default())
    }
}
