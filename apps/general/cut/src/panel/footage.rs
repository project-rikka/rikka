use cut_project::{Clip, Fade, FadeShape, Mix, MixField, Property, Rate, Treatment};
use rikka_app::prelude::*;
use rikka_effects::{Scale, Unit};

use super::keys::Keying;
use super::rows::{choice, group, number, sliding, sliding_with, switch_row};
use crate::wording;

pub(crate) fn picture(treatment: &Treatment, picture: Size) -> impl View {
    let roundest = (picture.width.min(picture.height) / 2.0).floor().max(1.0);
    group(
        tr!("cut.panel.picture"),
        vec![
            sliding(
                tr!("cut.panel.corner"),
                treatment.radius,
                Scale::new(0.0..=roundest).unit(Unit::Pixels),
                |treatment: &mut Treatment, radius: f32| treatment.radius = radius,
            ),
            switch_row(
                tr!("cut.panel.mirror_horizontal"),
                treatment.mirror.horizontal,
                |treatment: &mut Treatment, on: bool| treatment.mirror.horizontal = on,
            ),
            switch_row(
                tr!("cut.panel.mirror_vertical"),
                treatment.mirror.vertical,
                |treatment: &mut Treatment, on: bool| treatment.mirror.vertical = on,
            ),
        ],
    )
}

pub(crate) fn playing(treatment: &Treatment) -> impl View {
    group(
        tr!("cut.panel.playing"),
        vec![sliding(
            tr!("cut.panel.speed"),
            treatment.speed * 100.0,
            Scale::new(percent(Treatment::SPEEDS)).unit(Unit::Percent),
            |treatment: &mut Treatment, speed: f32| treatment.speed = speed / 100.0,
        )],
    )
}

pub(crate) fn sound(clip: &Clip, mix: Mix, keying: Keying, rate: Rate) -> impl View {
    let longest = clip.length() as f32 / rate.per_second() as f32;
    let seconds = || Scale::new(0.0..=longest).unit(Unit::Seconds).decimals(2);
    let frames = move |seconds: f32| (f64::from(seconds) * rate.per_second()).round() as i64;
    let length = |fade: Fade| fade.length as f32 / rate.per_second() as f32;
    group(
        tr!("cut.panel.sound"),
        vec![
            sliding_with(
                tr!("cut.panel.volume"),
                mix.volume * 100.0,
                Scale::new(percent(Mix::VOLUMES)).unit(Unit::Percent),
                |treatment: &mut Treatment, volume: f32| treatment.mix.volume = volume / 100.0,
                Some(keying.on(clip, Property::Mix(MixField::Volume))),
            ),
            sliding_with(
                tr!("cut.panel.pan"),
                mix.pan * 100.0,
                Scale::new(percent(Mix::PANS)),
                |treatment: &mut Treatment, pan: f32| treatment.mix.pan = pan / 100.0,
                Some(keying.on(clip, Property::Mix(MixField::Pan))),
            ),
            switch_row(
                tr!("cut.panel.muted"),
                mix.muted,
                |treatment: &mut Treatment, on: bool| treatment.mix.muted = on,
            ),
            number(
                tr!("cut.panel.fade_in"),
                length(mix.fade_in),
                seconds(),
                move |treatment: &mut Treatment, seconds: f32| {
                    treatment.mix.fade_in.length = frames(seconds);
                },
            ),
            choice(
                tr!("cut.panel.fade_in_shape"),
                FadeShape::ALL,
                mix.fade_in.shape,
                wording::fade_shape,
                |treatment: &mut Treatment, shape: FadeShape| treatment.mix.fade_in.shape = shape,
            ),
            number(
                tr!("cut.panel.fade_out"),
                length(mix.fade_out),
                seconds(),
                move |treatment: &mut Treatment, seconds: f32| {
                    treatment.mix.fade_out.length = frames(seconds);
                },
            ),
            choice(
                tr!("cut.panel.fade_out_shape"),
                FadeShape::ALL,
                mix.fade_out.shape,
                wording::fade_shape,
                |treatment: &mut Treatment, shape: FadeShape| treatment.mix.fade_out.shape = shape,
            ),
        ],
    )
}

fn percent(range: std::ops::RangeInclusive<f32>) -> std::ops::RangeInclusive<f32> {
    range.start() * 100.0..=range.end() * 100.0
}
