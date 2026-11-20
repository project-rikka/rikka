use std::sync::Arc;

use crate::animation::Tracked;
use crate::camera::CameraField;
use crate::clip::{Clip, ClipId, Source};
use crate::edit::Applied;
use crate::effect_slot::EffectSlot;
use crate::error::{Error, Result};
use crate::footage::{MixField, Treatment};
use crate::piece::{Content, Piece};
use crate::placement::{Placement, PlacementField};
use crate::project::Project;
use crate::property::Field;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Attribute {
    Placement,
    Picture,
    Speed,
    AnimationPresets,
    Effect(usize),
    Sound,
    Camera,
}

impl Attribute {
    #[must_use]
    pub fn is_effect(self) -> bool {
        matches!(self, Attribute::Effect(_))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KeyFit {
    #[default]
    Maintain,
    Stretch,
}

#[derive(Clone, Copy, Debug)]
struct Fit {
    from: i64,
    to: i64,
    keys: KeyFit,
}

impl Fit {
    fn ratio(self) -> Option<f64> {
        let stretches = self.keys == KeyFit::Stretch && self.from > 1 && self.to > 1;
        (stretches && self.from != self.to).then(|| (self.to - 1) as f64 / (self.from - 1) as f64)
    }

    fn at(self, ratio: f64) -> impl Fn(i64) -> i64 {
        let (from_last, to_last) = (self.from - 1, self.to - 1);
        move |at| match at >= from_last {
            true => to_last,
            false => ((at as f64 * ratio).round() as i64).clamp(0, to_last),
        }
    }

    fn tracked(self, track: &Tracked) -> Tracked {
        let span = 0..self.to;
        match self.ratio() {
            Some(ratio) => track.retimed(self.at(ratio), ratio).cropped(&span),
            None => track.cropped(&span),
        }
    }

    fn slot(self, slot: &EffectSlot) -> EffectSlot {
        let Some(ratio) = self.ratio() else {
            return slot.clone();
        };
        EffectSlot {
            tracks: slot
                .tracks
                .iter()
                .map(|(knob, track)| (knob.clone(), track.retimed(self.at(ratio), ratio)))
                .collect(),
            ..slot.clone()
        }
    }
}

impl Piece {
    #[must_use]
    pub fn attributes(&self) -> Vec<Attribute> {
        let mut offered = Vec::new();
        let visual = self.is_visual();
        if visual {
            offered.push(Attribute::Placement);
        }
        if let Content::Footage { media, .. } = &self.content
            && media.picture().is_some()
        {
            offered.extend([Attribute::Picture, Attribute::Speed]);
        }
        if visual {
            offered.push(Attribute::AnimationPresets);
            offered.extend((0..self.effects.len()).map(Attribute::Effect));
        }
        match &self.content {
            Content::Footage { media, .. } if media.sound().is_some() => {
                offered.push(Attribute::Sound);
            }
            Content::Camera(_) => offered.push(Attribute::Camera),
            Content::Footage { .. } | Content::Graphic(_) | Content::Region(_) => {}
        }
        offered
    }

    #[must_use]
    pub fn effects(&self) -> &[EffectSlot] {
        &self.effects
    }

    fn is_visual(&self) -> bool {
        match &self.content {
            Content::Footage { media, .. } => media.picture().is_some(),
            Content::Graphic(_) | Content::Region(_) => true,
            Content::Camera(_) => false,
        }
    }

    fn treatment(&self) -> Option<&Treatment> {
        match &self.content {
            Content::Footage { treatment, .. } => Some(treatment),
            Content::Graphic(_) | Content::Region(_) | Content::Camera(_) => None,
        }
    }

    fn placement(&self) -> Option<Placement> {
        match &self.content {
            Content::Footage { treatment, .. } => Some(treatment.placement),
            Content::Graphic(graphic) => Some(graphic.placement),
            Content::Region(region) => Some(region.placement),
            Content::Camera(_) => None,
        }
    }
}

impl Project {
    fn accepts(&self, clip: &Clip, attribute: Attribute) -> bool {
        match attribute {
            Attribute::Placement | Attribute::AnimationPresets | Attribute::Effect(_) => {
                self.is_visual(clip)
            }
            Attribute::Picture | Attribute::Speed => {
                clip.treatment().is_some() && self.is_visual(clip)
            }
            Attribute::Sound => self.has_sound(clip),
            Attribute::Camera => clip.camera().is_some(),
        }
    }

    #[must_use]
    pub fn pastable(&self, piece: &Piece, clips: &[ClipId]) -> Vec<Attribute> {
        let targets: Vec<&Clip> = self
            .editable(clips)
            .into_iter()
            .filter_map(|id| self.locate(id).map(|(_, clip)| clip))
            .collect();
        piece
            .attributes()
            .into_iter()
            .filter(|attribute| targets.iter().any(|clip| self.accepts(clip, *attribute)))
            .collect()
    }

    pub(crate) fn paste_attributes(
        &mut self,
        from: &Piece,
        onto: &[ClipId],
        attributes: &[Attribute],
        keys: KeyFit,
    ) -> Result<Applied> {
        self.check_unlocked(onto)?;
        let offered = from.attributes();
        let mut pasted = false;
        for id in onto {
            let (layer, clip) = self.locate(*id).ok_or(Error::UnknownClip)?;
            let accepted: Vec<Attribute> = offered
                .iter()
                .copied()
                .filter(|attribute| attributes.contains(attribute))
                .filter(|attribute| self.accepts(clip, *attribute))
                .collect();
            if accepted.is_empty() {
                continue;
            }
            let media_frames = clip.media().map_or(0, |media| self.media_frames(media));
            let room = self.layers[layer].next_start(*id);
            let clip = self
                .layer_mut(layer)
                .find_mut(*id)
                .ok_or(Error::UnknownClip)?;
            clip.adopt(from, &accepted, keys, media_frames, room);
            pasted = true;
        }
        match pasted {
            true => Ok(Applied::default()),
            false => Err(Error::NothingToPaste),
        }
    }

    pub(crate) fn remove_effects(&mut self, clips: &[ClipId]) -> Result<Applied> {
        self.check_unlocked(clips)?;
        let mut removed = false;
        for id in clips {
            let (_, clip) = self.locate(*id).ok_or(Error::UnknownClip)?;
            if clip.effects.is_empty() {
                continue;
            }
            self.held_mut(*id)?.effects.clear();
            removed = true;
        }
        match removed {
            true => Ok(Applied::default()),
            false => Err(Error::NoEffects),
        }
    }
}

impl Clip {
    fn adopt(
        &mut self,
        from: &Piece,
        accepted: &[Attribute],
        keys: KeyFit,
        media_frames: i64,
        room: Option<i64>,
    ) {
        if accepted.contains(&Attribute::Speed)
            && let Some(source) = from.treatment()
            && self
                .treatment()
                .is_some_and(|own| own.speed != source.speed)
        {
            let speed = source.speed;
            self.stretching(|clip| clip.retime(speed, media_frames, room));
        }
        let fit = Fit {
            from: from.length,
            to: self.length,
            keys,
        };
        let placement_fields = PlacementField::ALL.map(Field::Placement);
        let camera_fields = CameraField::ALL.map(Field::Camera);
        let mix_fields = MixField::ALL.map(Field::Mix);
        for attribute in accepted {
            match attribute {
                Attribute::Speed => {}
                Attribute::Placement => {
                    if let Some(placement) = from.placement() {
                        self.place_at(placement);
                    }
                    self.adopt_tracks(from, &placement_fields, true, fit);
                }
                Attribute::Picture => {
                    if let (Some(source), Source::Footage { treatment, .. }) =
                        (from.treatment(), &mut self.source)
                    {
                        treatment.radius = source.radius;
                        treatment.mirror = source.mirror;
                    }
                }
                Attribute::Sound => {
                    let length = self.length;
                    if let (Some(source), Source::Footage { treatment, .. }) =
                        (from.treatment(), &mut self.source)
                    {
                        treatment.mix = source.mix.fitted(length);
                    }
                    self.adopt_tracks(from, &mix_fields, false, fit);
                }
                Attribute::Camera => {
                    if let Content::Camera(camera) = &from.content {
                        self.source = Source::Camera(Arc::clone(camera));
                    }
                    self.adopt_tracks(from, &camera_fields, true, fit);
                }
                Attribute::AnimationPresets => {
                    self.animation_presets = from.animation_presets.checked();
                }
                Attribute::Effect(slot) => {
                    if let Some(slot) = from.effects.get(*slot) {
                        self.effects.push(fit.slot(slot).checked(self.length));
                    }
                }
            }
        }
    }

    fn adopt_tracks(&mut self, from: &Piece, fields: &[Field], tweens: bool, fit: Fit) {
        self.motion.keep(|field| !fields.contains(&field));
        let tweened = tweens
            && from
                .motion
                .tweening(fields.to_vec(), from.length - 1)
                .is_some();
        let last = self.length - 1;
        for (field, track) in from.motion.tracks() {
            if !fields.contains(field) {
                continue;
            }
            let fitted = match tweened {
                true => track.spanning(last),
                false => fit.tracked(track),
            };
            self.motion.insert(*field, fitted);
        }
    }
}
