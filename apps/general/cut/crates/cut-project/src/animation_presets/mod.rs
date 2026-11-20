mod animation_preset;
mod direction;
mod letters;
mod repeat;
mod transform;

pub use animation_preset::{AnimationPreset, AnimationPresetKind};
pub use direction::Direction;
pub(crate) use letters::mixed;
pub use letters::{Letters, Order, letter_count};
pub use repeat::{LoopKind, LoopPreset};
pub use transform::{PresetTransform, Wipe};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AnimationPresets {
    pub in_preset: Option<AnimationPreset>,
    pub out_preset: Option<AnimationPreset>,
    pub loop_preset: Option<LoopPreset>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PresetTransforms {
    pub whole: PresetTransform,
    pub letters: Vec<PresetTransform>,
}

#[derive(Clone, Copy, Debug)]
enum PresetSide {
    In,
    Out,
}

impl AnimationPresets {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.in_preset.is_none() && self.out_preset.is_none() && self.loop_preset.is_none()
    }

    #[must_use]
    pub fn lengths(&self, length: i64) -> (i64, i64) {
        let enter = self
            .in_preset
            .map_or(0, |animation_preset| animation_preset.length.max(0));
        let exit = self
            .out_preset
            .map_or(0, |animation_preset| animation_preset.length.max(0));
        let length = length.max(0);
        match enter + exit <= length {
            true => (enter, exit),
            false => {
                let shortened = enter * length / (enter + exit);
                (shortened, length - shortened)
            }
        }
    }

    #[must_use]
    pub fn transforms_at(&self, at: f64, length: i64, letters: usize) -> PresetTransforms {
        let lettered = |by_letter: Option<Letters>| letters > 0 && by_letter.is_some();
        let (enter_length, exit_length) = self.lengths(length);
        let mut whole = PresetTransform::REST;
        let mut each = vec![PresetTransform::REST; letters];
        let animation_presets = [
            (self.in_preset, PresetSide::In, enter_length),
            (self.out_preset, PresetSide::Out, exit_length),
        ];
        for (animation_preset, side, span) in animation_presets {
            let Some(animation_preset) = animation_preset else {
                continue;
            };
            let start = match side {
                PresetSide::In => 0.0,
                PresetSide::Out => (length - span) as f64,
            };
            match (lettered(animation_preset.letters), animation_preset.letters) {
                (true, Some(by_letter)) => {
                    let ranks = by_letter.ranks(letters);
                    let (window, step) = windows(span as f64, f64::from(by_letter.delay), &ranks);
                    for (letter, rank) in each.iter_mut().zip(ranks) {
                        let begun = start + rank as f64 * step;
                        *letter = letter.then(side_transform(
                            &animation_preset,
                            side,
                            at - begun,
                            window,
                        ));
                    }
                }
                _ => {
                    whole = whole.then(side_transform(
                        &animation_preset,
                        side,
                        at - start,
                        span as f64,
                    ))
                }
            }
        }
        if let Some(repeat) = self.loop_preset {
            let at = at + repeat.phase as f64;
            match (lettered(repeat.letters), repeat.letters) {
                (true, Some(by_letter)) => {
                    for (letter, rank) in each.iter_mut().zip(by_letter.ranks(letters)) {
                        let lag = rank as f64 * f64::from(by_letter.delay);
                        *letter = letter.then(repeat.transform_at(at - lag));
                    }
                }
                _ => whole = whole.then(repeat.transform_at(at)),
            }
        }
        let any_lettered = lettered(
            self.in_preset
                .and_then(|animation_preset| animation_preset.letters),
        ) || lettered(
            self.out_preset
                .and_then(|animation_preset| animation_preset.letters),
        ) || lettered(self.loop_preset.and_then(|repeat| repeat.letters));
        PresetTransforms {
            whole,
            letters: match any_lettered {
                true => each,
                false => Vec::new(),
            },
        }
    }

    pub(crate) fn checked(self) -> AnimationPresets {
        AnimationPresets {
            in_preset: self.in_preset.map(AnimationPreset::checked),
            out_preset: self.out_preset.map(AnimationPreset::checked),
            loop_preset: self.loop_preset.map(LoopPreset::checked),
        }
    }

    pub(crate) fn headed(&mut self, frames: i64) {
        if let Some(repeat) = &mut self.loop_preset {
            repeat.phase += frames;
        }
    }
}

fn windows(span: f64, delay: f64, ranks: &[usize]) -> (f64, f64) {
    let last = ranks.iter().copied().max().unwrap_or(0) as f64;
    if last == 0.0 {
        return (span, 0.0);
    }
    let window = (span - delay * last).max(span / (last + 1.0));
    (window, (span - window) / last)
}

fn side_transform(
    animation_preset: &AnimationPreset,
    side: PresetSide,
    elapsed: f64,
    span: f64,
) -> PresetTransform {
    let progress = match span > 0.0 {
        true => (elapsed / span).clamp(0.0, 1.0),
        false => 1.0,
    };
    let rest = match side {
        PresetSide::In => animation_preset.easing.at(progress),
        PresetSide::Out if span > 0.0 => 1.0 - animation_preset.easing.at(progress),
        PresetSide::Out => 1.0,
    };
    animation_preset.kind.transform_at(
        rest as f32,
        animation_preset.strength,
        animation_preset.direction,
    )
}
