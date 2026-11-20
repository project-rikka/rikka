use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::sync::Arc;

use rikka_effects::Unit;

use crate::animation_presets::{
    AnimationPreset, AnimationPresetKind, AnimationPresets, Letters, LoopKind, LoopPreset,
};
use crate::camera::CameraField;
use crate::clip::{Clip, Source};
use crate::edit::Applied;
use crate::effect_slot::EffectSlot;
use crate::error::{Error, Result};
use crate::markers::Marker;
use crate::media::MediaId;
use crate::placement::{Placement, PlacementField};
use crate::project::{Format, Marks, Project};
use crate::property::Field;
use crate::rate::Rate;
use crate::region::Area;
use crate::transitions::{Ends, Transition, TransitionKnob};

#[derive(Clone, Copy, Debug)]
struct Retime {
    numerator: i128,
    denominator: i128,
}

impl Retime {
    fn between(from: Rate, to: Rate) -> Retime {
        Retime {
            numerator: i128::from(to.numerator()) * i128::from(from.denominator()),
            denominator: i128::from(to.denominator()) * i128::from(from.numerator()),
        }
    }

    fn is_same(self) -> bool {
        self.numerator == self.denominator
    }

    fn ratio(self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }

    fn frames(self, frames: i64) -> i64 {
        let scaled = 2 * i128::from(frames.abs()) * self.numerator;
        let rounded = (scaled + self.denominator) / (2 * self.denominator);
        (rounded.min(i128::from(i64::MAX)) as i64) * frames.signum()
    }

    fn lasting(self, frames: i64) -> i64 {
        match frames > 0 {
            true => self.frames(frames).max(1),
            false => 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Sizing {
    scale: f32,
    local: f32,
}

impl Sizing {
    fn is_same(self) -> bool {
        self.scale == 1.0 && self.local == 1.0
    }

    fn factor_for(self, field: Field) -> f32 {
        match field {
            Field::Placement(PlacementField::Position | PlacementField::Depth) => self.scale,
            Field::Placement(PlacementField::Anchor) => self.local,
            Field::Art(field) if field.is_length() => self.scale,
            Field::Camera(CameraField::Interest | CameraField::Dolly) => self.scale,
            Field::Placement(_) | Field::Art(_) | Field::Camera(_) | Field::Mix(_) => 1.0,
        }
    }
}

impl Format {
    pub const SIDES: RangeInclusive<u32> = 16..=16_384;
}

impl Project {
    pub(crate) fn reformat(&mut self, format: Format) -> Result<Applied> {
        if !Format::SIDES.contains(&format.width) || !Format::SIDES.contains(&format.height) {
            return Err(Error::FrameSize {
                width: format.width,
                height: format.height,
            });
        }
        let from = self.format;
        let retime = Retime::between(from.rate, format.rate);
        let scale = (format.width as f32 / from.width as f32)
            .min(format.height as f32 / from.height as f32);
        let frames: BTreeMap<MediaId, i64> = self
            .media
            .iter()
            .map(|(id, media)| (*id, media.frames(format.rate)))
            .collect();
        let locals: BTreeMap<MediaId, f32> = self
            .media
            .iter()
            .filter_map(|(id, media)| {
                let picture = media.picture()?;
                let before = from.fitted(picture.width, picture.height).width;
                let after = format.fitted(picture.width, picture.height).width;
                Some((*id, after / before.max(f32::EPSILON)))
            })
            .collect();
        self.format = format;
        self.fitted = true;
        if retime.is_same() && scale == 1.0 && locals.values().all(|local| *local == 1.0) {
            return Ok(Applied::default());
        }
        for layer in &mut self.layers {
            if layer.clips.is_empty() {
                continue;
            }
            let layer = Arc::make_mut(layer);
            for clip in &mut layer.clips {
                let media = clip.media();
                let local = media
                    .and_then(|media| locals.get(&media))
                    .copied()
                    .unwrap_or(scale);
                clip.rescaled(Sizing { scale, local });
                clip.retimed(retime, media.and_then(|media| frames.get(&media)).copied());
            }
            let mut floor = 0;
            for clip in &mut layer.clips {
                clip.start = clip.start.max(floor);
                floor = clip.end();
            }
        }
        self.markers = self
            .markers
            .remapped(|marker| retimed_marker(marker, retime));
        self.marks = Marks {
            mark_in: self.marks.mark_in.map(|at| retime.frames(at)),
            mark_out: self
                .marks
                .mark_out
                .map(|at| (retime.frames(at + 1) - 1).max(0)),
        };
        Ok(Applied::default())
    }
}

impl Clip {
    fn retimed(&mut self, retime: Retime, media_frames: Option<i64>) {
        if retime.is_same() {
            return;
        }
        let (start, last) = (self.start, self.length - 1);
        let first = retime.frames(start);
        let mut length = (retime.frames(start + self.length) - first).max(1);
        if let Some(media) = media_frames {
            let speed = self.speed();
            let reach = |length: i64| (length as f64 * speed).ceil() as i64;
            let source_in = retime
                .frames(self.source_in())
                .min(media - reach(length))
                .max(0);
            length = length
                .min(((media - source_in) as f64 / speed).floor() as i64)
                .max(1);
            if let Source::Footage {
                source_in: current_in,
                ..
            } = &mut self.source
            {
                *current_in = source_in;
            }
        }
        let key = |at: i64| match at >= last {
            true => length - 1,
            false => (retime.frames(start + at) - first).clamp(0, length - 1),
        };
        let ratio = retime.ratio();
        self.motion = self.motion.remapped(|_, track| track.retimed(key, ratio));
        for slot in &mut self.effects {
            for track in slot.tracks.values_mut() {
                *track = track.retimed(key, ratio);
            }
        }
        self.animation_presets = retimed_animation_presets(self.animation_presets, retime);
        self.transitions = Ends {
            into: self
                .transitions
                .into
                .map(|transition| retimed_transition(transition, retime)),
            out: self
                .transitions
                .out
                .map(|transition| retimed_transition(transition, retime)),
        };
        self.markers = self
            .markers
            .remapped(|marker| retimed_marker(marker, retime));
        if let Source::Footage { treatment, .. } = &mut self.source {
            treatment.mix.fade_in = treatment.mix.fade_in.retimed(|at| retime.frames(at));
            treatment.mix.fade_out = treatment.mix.fade_out.retimed(|at| retime.frames(at));
            treatment.mix = treatment.mix.fitted(length);
        }
        self.start = first;
        self.length = length;
    }

    fn rescaled(&mut self, sizing: Sizing) {
        if sizing.is_same() {
            return;
        }
        let Sizing { scale, local } = sizing;
        let placement = self.placement();
        self.place_at(Placement {
            x: placement.x * scale,
            y: placement.y * scale,
            depth: placement.depth * scale,
            anchor: placement.anchor * local,
            ..placement
        });
        match &mut self.source {
            Source::Footage { treatment, .. } => treatment.radius *= local,
            Source::Graphic(graphic) => {
                let graphic = Arc::make_mut(graphic);
                graphic.art = graphic.art.scaled(scale);
            }
            Source::Region(region) => {
                let region = Arc::make_mut(region);
                region.area = scaled_area(region.area, scale);
                region.feather *= scale;
            }
            Source::Camera(camera) => {
                let camera = Arc::make_mut(camera);
                camera.interest = camera.interest * scale;
                camera.dolly *= scale;
            }
        }
        self.motion = self.motion.remapped(|field, track| {
            let factor = sizing.factor_for(field);
            match factor == 1.0 {
                true => track.clone(),
                false => track.scaled(factor),
            }
        });
        self.effects = self
            .effects
            .iter()
            .map(|slot| scaled_slot(slot, local))
            .collect();
        self.animation_presets = scaled_animation_presets(self.animation_presets, scale);
        self.transitions = Ends {
            into: self
                .transitions
                .into
                .map(|transition| scaled_transition(transition, scale)),
            out: self
                .transitions
                .out
                .map(|transition| scaled_transition(transition, scale)),
        };
    }
}

fn retimed_marker(marker: &Marker, retime: Retime) -> Marker {
    Marker {
        at: retime.frames(marker.at),
        length: retime.frames(marker.length),
        ..marker.clone()
    }
}

fn retimed_animation_presets(
    animation_presets: AnimationPresets,
    retime: Retime,
) -> AnimationPresets {
    let letters = |letters: Option<Letters>| {
        letters.map(|by_letter| Letters {
            delay: by_letter.delay * retime.ratio() as f32,
            ..by_letter
        })
    };
    let animation_preset = |animation_preset: AnimationPreset| AnimationPreset {
        length: retime.lasting(animation_preset.length),
        letters: letters(animation_preset.letters),
        ..animation_preset
    };
    AnimationPresets {
        in_preset: animation_presets.in_preset.map(animation_preset),
        out_preset: animation_presets.out_preset.map(animation_preset),
        loop_preset: animation_presets.loop_preset.map(|repeat| LoopPreset {
            period: retime.lasting(repeat.period).max(1),
            phase: retime.frames(repeat.phase),
            letters: letters(repeat.letters),
            ..repeat
        }),
    }
}

fn retimed_transition(transition: Transition, retime: Retime) -> Transition {
    let before = retime.frames(transition.before);
    let after = retime.frames(transition.after);
    let after = match before + after {
        0 => 1,
        _ => after,
    };
    Transition {
        before,
        after,
        ..transition
    }
}

fn scaled_area(area: Area, factor: f32) -> Area {
    match area {
        Area::Whole => Area::Whole,
        Area::Shape {
            shape,
            width,
            height,
        } => Area::Shape {
            shape: match shape {
                rikka_figure::Shape::Rectangle { radius } => rikka_figure::Shape::Rectangle {
                    radius: radius * factor,
                },
                other => other,
            },
            width: width * factor,
            height: height * factor,
        },
    }
}

fn scaled_slot(slot: &EffectSlot, factor: f32) -> EffectSlot {
    let mut scaled = slot.clone();
    for knob in slot.effect.knobs() {
        if knob.scale.unit != Unit::Pixels {
            continue;
        }
        if let Some(value) = slot.effect.value(knob.key) {
            scaled
                .effect
                .set(knob.key, knob.scale.clamp(value * factor));
        }
        if let Some(track) = scaled.tracks.get_mut(knob.key) {
            *track = track.scaled(factor);
            track.map_values(|value| knob.scale.clamp(value));
        }
    }
    scaled
}

fn scaled_animation_presets(animation_presets: AnimationPresets, factor: f32) -> AnimationPresets {
    let animation_preset = |animation_preset: AnimationPreset| match animation_preset.kind {
        AnimationPresetKind::Slide | AnimationPresetKind::Blur => AnimationPreset {
            strength: animation_preset.strength * factor,
            ..animation_preset
        },
        _ => animation_preset,
    };
    AnimationPresets {
        in_preset: animation_presets.in_preset.map(animation_preset),
        out_preset: animation_presets.out_preset.map(animation_preset),
        loop_preset: animation_presets
            .loop_preset
            .map(|repeat| match repeat.kind {
                LoopKind::Shake | LoopKind::Float => LoopPreset {
                    strength: repeat.strength * factor,
                    ..repeat
                },
                LoopKind::Pulse | LoopKind::Spin => repeat,
            }),
    }
}

fn scaled_transition(transition: Transition, factor: f32) -> Transition {
    let tuning = [
        TransitionKnob::Amount,
        TransitionKnob::Blur,
        TransitionKnob::Size,
    ]
    .into_iter()
    .fold(transition.tuning, |tuning, knob| {
        match (knob.scale(transition.kind), tuning.value(knob)) {
            (Some(scale), Some(value)) if scale.unit == Unit::Pixels => {
                tuning.with(transition.kind, knob, value * factor)
            }
            _ => tuning,
        }
    });
    Transition {
        tuning,
        ..transition
    }
}
