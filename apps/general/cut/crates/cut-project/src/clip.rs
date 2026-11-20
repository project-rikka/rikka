use std::ops::Range;
use std::sync::Arc;

use rikka_effects::Effect;

use crate::animation::{Channel, Value};
use crate::animation_presets::AnimationPresets;
use crate::camera::{Camera, CameraField};
use crate::effect_slot::{EffectSlot, sampled};
use crate::footage::{Level, Mix, MixField, Treatment};
use crate::graphic::{Art, Graphic};
use crate::markers::Markers;
use crate::media::MediaId;
use crate::motion::Motion;
use crate::placement::{Placement, PlacementField};
use crate::property::{Animated, Field, Property};
use crate::region::Region;
use crate::transitions::Ends;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClipId(pub(crate) u64);

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Footage {
        media: MediaId,
        source_in: i64,
        treatment: Treatment,
    },
    Graphic(Arc<Graphic>),
    Region(Arc<Region>),
    Camera(Arc<Camera>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub(crate) id: ClipId,
    pub(crate) start: i64,
    pub(crate) length: i64,
    pub(crate) source: Source,
    pub(crate) motion: Motion,
    pub(crate) effects: Vec<EffectSlot>,
    pub(crate) animation_presets: AnimationPresets,
    pub(crate) transitions: Ends,
    pub(crate) markers: Markers,
}

impl Clip {
    pub(crate) fn new(id: ClipId, start: i64, length: i64, source: Source) -> Clip {
        Clip {
            id,
            start,
            length,
            source,
            motion: Motion::default(),
            effects: Vec::new(),
            animation_presets: AnimationPresets::default(),
            transitions: Ends::default(),
            markers: Markers::default(),
        }
    }

    #[must_use]
    pub fn id(&self) -> ClipId {
        self.id
    }

    #[must_use]
    pub fn source(&self) -> &Source {
        &self.source
    }

    #[must_use]
    pub fn media(&self) -> Option<MediaId> {
        match &self.source {
            Source::Footage { media, .. } => Some(*media),
            Source::Graphic(_) | Source::Region(_) | Source::Camera(_) => None,
        }
    }

    #[must_use]
    pub fn graphic(&self) -> Option<&Arc<Graphic>> {
        match &self.source {
            Source::Graphic(graphic) => Some(graphic),
            Source::Footage { .. } | Source::Region(_) | Source::Camera(_) => None,
        }
    }

    #[must_use]
    pub fn region(&self) -> Option<&Arc<Region>> {
        match &self.source {
            Source::Region(region) => Some(region),
            Source::Footage { .. } | Source::Graphic(_) | Source::Camera(_) => None,
        }
    }

    #[must_use]
    pub fn camera(&self) -> Option<&Arc<Camera>> {
        match &self.source {
            Source::Camera(camera) => Some(camera),
            Source::Footage { .. } | Source::Graphic(_) | Source::Region(_) => None,
        }
    }

    #[must_use]
    pub fn treatment(&self) -> Option<&Treatment> {
        match &self.source {
            Source::Footage { treatment, .. } => Some(treatment),
            Source::Graphic(_) | Source::Region(_) | Source::Camera(_) => None,
        }
    }

    #[must_use]
    pub fn placement(&self) -> Placement {
        match &self.source {
            Source::Footage { treatment, .. } => treatment.placement,
            Source::Graphic(graphic) => graphic.placement,
            Source::Region(region) => region.placement,
            Source::Camera(_) => Placement::default(),
        }
    }

    #[must_use]
    pub fn placement_at(&self, frame: i64) -> Placement {
        self.motion
            .animated(&self.placement(), self.local(frame) as f64)
            .checked()
    }

    #[must_use]
    pub fn art_at(&self, frame: i64) -> Option<Art> {
        let graphic = self.graphic()?;
        Some(self.motion.animated(&graphic.art, self.local(frame) as f64))
    }

    #[must_use]
    pub fn camera_at(&self, frame: i64) -> Option<Camera> {
        let camera = self.camera()?;
        Some(
            self.motion
                .animated(&**camera, self.local(frame) as f64)
                .checked(),
        )
    }

    #[must_use]
    pub fn mix_at(&self, frame: i64) -> Option<Mix> {
        self.audible_mix(frame as f64, None)
    }

    #[must_use]
    pub fn treatment_at(&self, frame: i64) -> Option<Treatment> {
        Some(Treatment {
            placement: self.placement_at(frame),
            mix: self.mix_at(frame)?,
            ..*self.treatment()?
        })
    }

    #[must_use]
    pub fn level_at(&self, at: f64, preview: Option<Mix>) -> Option<Level> {
        let mix = self.audible_mix(at, preview)?;
        Some(mix.level(at - self.start as f64, self.length))
    }

    #[must_use]
    pub fn audible_mix(&self, at: f64, preview: Option<Mix>) -> Option<Mix> {
        let base = self.treatment()?.mix;
        let local = (at - self.start as f64).clamp(0.0, (self.length - 1) as f64);
        Some(preview.unwrap_or_else(|| self.motion.animated(&base, local)))
    }

    #[must_use]
    pub fn animation_presets(&self) -> &AnimationPresets {
        &self.animation_presets
    }

    #[must_use]
    pub fn transitions(&self) -> &Ends {
        &self.transitions
    }

    #[must_use]
    pub fn effects(&self) -> &[EffectSlot] {
        &self.effects
    }

    #[must_use]
    pub fn effects_at(&self, frame: i64) -> Vec<Effect> {
        sampled(&self.effects, self.local(frame) as f64)
    }

    #[must_use]
    pub fn retuned(&self, slot: usize, effect: Effect, frame: i64) -> Option<Vec<EffectSlot>> {
        let mut effects = self.effects.clone();
        effects.get_mut(slot)?.tuned(effect, self.local(frame));
        Some(effects)
    }

    #[must_use]
    pub fn channel(&self, property: &Property) -> Option<Channel<'_>> {
        match property {
            Property::Effect { slot, knob } => {
                self.effects.get(*slot)?.track(knob).map(Channel::Scalar)
            }
            _ => self.motion.channel(Field::from_property(property)?),
        }
    }

    #[must_use]
    pub fn is_keyed(&self, property: &Property) -> bool {
        self.channel(property).is_some()
    }

    #[must_use]
    pub fn value_at(&self, property: &Property, frame: i64) -> Option<Value> {
        match property {
            Property::Placement(field) => Some(self.placement_at(frame).value(*field)),
            Property::Art(field) => self.art_at(frame)?.value(*field),
            Property::Camera(field) => Some(self.camera_at(frame)?.value(*field)),
            Property::Mix(field) => Some(Value::Scalar(self.mix_at(frame)?.value(*field))),
            Property::Effect { slot, knob } => self
                .effects
                .get(*slot)?
                .effect_at(self.local(frame) as f64)
                .value(knob)
                .map(Value::Scalar),
        }
    }

    #[must_use]
    pub fn properties(&self) -> Vec<Property> {
        let mut properties = self.grouped();
        if let Some(graphic) = self.graphic() {
            properties.extend(graphic.art.fields().into_iter().map(Property::Art));
        }
        for (slot, effect_slot) in self.effects.iter().enumerate() {
            properties.extend(
                effect_slot
                    .effect
                    .knobs()
                    .iter()
                    .map(|knob| Property::Effect {
                        slot,
                        knob: knob.key.to_owned(),
                    }),
            );
        }
        if self.treatment().is_some() {
            properties.extend(MixField::ALL.map(Property::Mix));
        }
        properties
    }

    #[must_use]
    pub fn grouped(&self) -> Vec<Property> {
        match &self.source {
            Source::Camera(_) => CameraField::ALL.map(Property::Camera).to_vec(),
            Source::Footage { .. } | Source::Graphic(_) | Source::Region(_) => {
                let three_d = self.placement().three_d;
                PlacementField::ALL
                    .into_iter()
                    .filter(|field| three_d || !field.needs_three_d())
                    .map(Property::Placement)
                    .collect()
            }
        }
    }

    #[must_use]
    pub fn is_animated(&self) -> bool {
        !self.motion.is_still() || self.effects.iter().any(EffectSlot::is_animated)
    }

    #[must_use]
    pub fn is_moving(&self) -> bool {
        self.motion.moves() || self.effects.iter().any(EffectSlot::is_animated)
    }

    #[must_use]
    pub fn keyed_frames(&self) -> Vec<i64> {
        let effects = self
            .effects
            .iter()
            .flat_map(|slot| slot.tracks.values())
            .flat_map(|track| track.keys().iter().map(|key| key.at));
        let mut keyed: Vec<i64> = self
            .motion
            .frames()
            .chain(effects)
            .map(|at| self.start + at)
            .collect();
        keyed.sort_unstable();
        keyed.dedup();
        keyed
    }

    #[must_use]
    pub fn start(&self) -> i64 {
        self.start
    }

    #[must_use]
    pub fn end(&self) -> i64 {
        self.start + self.length
    }

    #[must_use]
    pub fn length(&self) -> i64 {
        self.length
    }

    #[must_use]
    pub fn local(&self, frame: i64) -> i64 {
        (frame - self.start).clamp(0, self.length - 1)
    }

    #[must_use]
    pub fn source_in(&self) -> i64 {
        match &self.source {
            Source::Footage { source_in, .. } => *source_in,
            Source::Graphic(_) | Source::Region(_) | Source::Camera(_) => 0,
        }
    }

    #[must_use]
    pub fn speed(&self) -> f64 {
        self.treatment()
            .map_or(1.0, |treatment| f64::from(treatment.speed))
    }

    #[must_use]
    pub fn span(&self) -> Range<i64> {
        self.start..self.end()
    }

    #[must_use]
    pub fn covers(&self, frame: i64) -> bool {
        self.span().contains(&frame)
    }

    #[must_use]
    pub fn origin(&self) -> f64 {
        self.source_in() as f64 - self.start as f64 * self.speed()
    }

    #[must_use]
    pub fn source_position(&self, frame: f64) -> f64 {
        self.origin() + frame * self.speed()
    }

    #[must_use]
    pub fn source_at(&self, frame: i64) -> i64 {
        self.source_in() + ((frame - self.start) as f64 * self.speed()).floor() as i64
    }

    #[must_use]
    pub(crate) fn source_span(&self) -> i64 {
        (self.length as f64 * self.speed()).ceil() as i64
    }

    pub(crate) fn frame_reaching(&self, source: i64) -> i64 {
        self.start + ((source - self.source_in()) as f64 / self.speed()).ceil() as i64
    }
    pub(crate) fn overlaps(&self, range: &Range<i64>) -> bool {
        self.start < range.end && range.start < self.end()
    }

    pub(crate) fn base(&self, field: Field) -> Option<Value> {
        match (&self.source, field) {
            (Source::Camera(camera), Field::Camera(_)) => camera.field_value(field),
            (Source::Graphic(graphic), Field::Art(_)) => graphic.art.field_value(field),
            (Source::Footage { treatment, .. }, Field::Mix(_)) => treatment.mix.field_value(field),
            (Source::Camera(_), _) | (_, Field::Camera(_) | Field::Art(_) | Field::Mix(_)) => None,
            (_, Field::Placement(field)) => Some(self.placement().value(field)),
        }
    }

    pub(crate) fn rebase(&mut self, field: Field, value: Value) {
        let value = field.clamp_value(value);
        match (&mut self.source, field) {
            (Source::Camera(camera), Field::Camera(_)) => {
                let camera_mut = Arc::make_mut(camera);
                *camera_mut = camera_mut.with_field(field, value).checked();
            }
            (Source::Graphic(graphic), Field::Art(_)) => {
                let graphic_mut = Arc::make_mut(graphic);
                graphic_mut.art = graphic_mut.art.with_field(field, value);
            }
            (Source::Footage { treatment, .. }, Field::Mix(_)) => {
                treatment.mix = treatment.mix.with_field(field, value);
            }
            (Source::Camera(_), _) | (_, Field::Camera(_) | Field::Art(_) | Field::Mix(_)) => {}
            (_, Field::Placement(field)) => {
                let placement = self.placement().with(field, value).checked();
                self.place_at(placement);
            }
        }
    }

    pub(crate) fn place_at(&mut self, placement: Placement) {
        match &mut self.source {
            Source::Footage { treatment, .. } => treatment.placement = placement,
            Source::Graphic(graphic) => Arc::make_mut(graphic).placement = placement,
            Source::Region(region) => Arc::make_mut(region).placement = placement,
            Source::Camera(_) => {}
        }
    }

    pub(crate) fn settled<T: Animated>(&mut self, base: &T, target: &T, frame: i64) -> T {
        let at = self.local(frame);
        let tweening = self.tweening();
        self.motion.absorbed(base, target, at, tweening.as_ref())
    }

    pub(crate) fn applies(&self, field: Field) -> bool {
        self.base(field).is_some()
    }

    pub(crate) fn prune(&mut self) {
        let fields: Vec<Field> = self.motion.tracks().keys().copied().collect();
        let retained: Vec<Field> = fields
            .into_iter()
            .filter(|field| self.applies(*field))
            .collect();
        self.motion.keep(|field| retained.contains(&field));
    }

    pub(crate) fn cut_head(&mut self, frames: i64) {
        let cut = self.source_at(self.start + frames);
        self.crop_keys(&(frames..self.length));
        self.animation_presets.headed(frames);
        self.start += frames;
        self.length -= frames;
        if let Source::Footage { source_in, .. } = &mut self.source {
            *source_in = cut;
        }
    }

    pub(crate) fn cut_tail(&mut self, frames: i64) {
        self.length -= frames;
        self.crop_keys(&(0..self.length));
    }

    pub(crate) fn divided(&mut self, fresh: ClipId, at: i64) -> Clip {
        let mut right = self.clone();
        right.id = fresh;
        right.cut_head(at - self.start);
        right.animation_presets.in_preset = None;
        right.transitions.into = None;
        self.cut_tail(self.end() - at);
        self.animation_presets.out_preset = None;
        self.transitions.out = None;
        right
    }

    pub(crate) fn slip(&mut self, frames: i64) {
        if let Source::Footage { source_in, .. } = &mut self.source {
            *source_in += frames;
        }
    }

    pub(crate) fn retime(&mut self, speed: f32, media_frames: i64, room: Option<i64>) {
        let covered = self.length as f64 * self.speed();
        if let Source::Footage { treatment, .. } = &mut self.source {
            treatment.speed = speed;
        }
        let speed = self.speed();
        let covering_length = (covered / speed).round() as i64;
        let available = ((media_frames - self.source_in()) as f64 / speed).floor() as i64;
        let fits = room.map_or(i64::MAX, |room| room - self.start);
        self.length = covering_length.min(available).min(fits).max(1);
        self.crop_keys(&(0..self.length));
    }

    fn crop_keys(&mut self, span: &Range<i64>) {
        self.motion = self.motion.cropped(span);
        self.effects = self.effects.iter().map(|slot| slot.cropped(span)).collect();
        if let Source::Footage { treatment, .. } = &mut self.source {
            treatment.mix = treatment.mix.fitted(span.end - span.start);
        }
    }
}
