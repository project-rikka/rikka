use cut_project::{
    AnimationPreset, AnimationPresetKind, AnimationPresets, ClipId, Direction, Edit, Letters,
    LoopKind, LoopPreset,
};
use rikka_app::prelude::*;
use rikka_keyframe::Easing;

use super::{Cut, Popup};
use crate::easing::Picking;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum PresetSlot {
    In,
    Out,
    Loop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum PresetStyle {
    InOut(AnimationPresetKind),
    Loop(LoopKind),
}

pub(crate) trait Part {
    fn span(&mut self) -> &mut i64;
    fn easing(&mut self) -> &mut Easing;
    fn strength(&mut self) -> &mut f32;
    fn direction(&mut self) -> &mut Direction;
    fn letters(&mut self) -> &mut Option<Letters>;
}

impl Part for AnimationPreset {
    fn span(&mut self) -> &mut i64 {
        &mut self.length
    }

    fn easing(&mut self) -> &mut Easing {
        &mut self.easing
    }

    fn strength(&mut self) -> &mut f32 {
        &mut self.strength
    }

    fn direction(&mut self) -> &mut Direction {
        &mut self.direction
    }

    fn letters(&mut self) -> &mut Option<Letters> {
        &mut self.letters
    }
}

impl Part for LoopPreset {
    fn span(&mut self) -> &mut i64 {
        &mut self.period
    }

    fn easing(&mut self) -> &mut Easing {
        &mut self.easing
    }

    fn strength(&mut self) -> &mut f32 {
        &mut self.strength
    }

    fn direction(&mut self) -> &mut Direction {
        &mut self.direction
    }

    fn letters(&mut self) -> &mut Option<Letters> {
        &mut self.letters
    }
}

impl PresetSlot {
    pub(crate) const ALL: [PresetSlot; 3] = [PresetSlot::In, PresetSlot::Out, PresetSlot::Loop];

    pub(crate) fn styles(self, lettered: bool) -> Vec<PresetStyle> {
        match self {
            PresetSlot::In | PresetSlot::Out => AnimationPresetKind::ALL
                .into_iter()
                .filter(|kind| lettered || *kind != AnimationPresetKind::Typewriter)
                .map(PresetStyle::InOut)
                .collect(),
            PresetSlot::Loop => LoopKind::ALL.map(PresetStyle::Loop).to_vec(),
        }
    }

    pub(crate) fn style(self, animation_presets: &AnimationPresets) -> Option<PresetStyle> {
        match self {
            PresetSlot::In => animation_presets
                .in_preset
                .map(|animation_preset| PresetStyle::InOut(animation_preset.kind)),
            PresetSlot::Out => animation_presets
                .out_preset
                .map(|animation_preset| PresetStyle::InOut(animation_preset.kind)),
            PresetSlot::Loop => animation_presets
                .loop_preset
                .map(|repeat| PresetStyle::Loop(repeat.kind)),
        }
    }

    pub(crate) fn part(self, animation_presets: &mut AnimationPresets) -> Option<&mut dyn Part> {
        match self {
            PresetSlot::In => animation_presets
                .in_preset
                .as_mut()
                .map(|animation_preset| animation_preset as &mut dyn Part),
            PresetSlot::Out => animation_presets
                .out_preset
                .as_mut()
                .map(|animation_preset| animation_preset as &mut dyn Part),
            PresetSlot::Loop => animation_presets
                .loop_preset
                .as_mut()
                .map(|repeat| repeat as &mut dyn Part),
        }
    }

    pub(crate) fn read<T>(
        self,
        animation_presets: &AnimationPresets,
        read: impl FnOnce(&mut dyn Part) -> T,
    ) -> Option<T> {
        let mut edited = *animation_presets;
        self.part(&mut edited).map(read)
    }

    pub(crate) fn easing(self, animation_presets: &AnimationPresets) -> Option<Easing> {
        self.read(animation_presets, |part| *part.easing())
    }

    pub(crate) fn tune(
        self,
        animation_presets: &mut AnimationPresets,
        tune: impl FnOnce(&mut dyn Part),
    ) {
        if let Some(part) = self.part(animation_presets) {
            tune(part);
        }
    }

    pub(crate) fn begin(
        self,
        animation_presets: &mut AnimationPresets,
        style: Option<PresetStyle>,
    ) {
        let current = self.read(animation_presets, |part| (*part.span(), *part.letters()));
        match (self, style) {
            (PresetSlot::In, Some(PresetStyle::InOut(kind))) => {
                animation_presets.in_preset = Some(AnimationPreset::new_in(kind));
            }
            (PresetSlot::Out, Some(PresetStyle::InOut(kind))) => {
                animation_presets.out_preset = Some(AnimationPreset::new_out(kind));
            }
            (PresetSlot::Loop, Some(PresetStyle::Loop(cycle))) => {
                animation_presets.loop_preset = Some(LoopPreset::new(cycle))
            }
            (PresetSlot::In, _) => animation_presets.in_preset = None,
            (PresetSlot::Out, _) => animation_presets.out_preset = None,
            (PresetSlot::Loop, _) => animation_presets.loop_preset = None,
        }
        if let Some((span, letters)) = current {
            self.tune(animation_presets, |part| {
                if !matches!(self, PresetSlot::Loop) {
                    *part.span() = span;
                }
                *part.letters() = letters;
            });
        }
    }
}

impl Cut {
    pub(crate) fn preview_animation_presets(&mut self, change: impl FnOnce(&mut AnimationPresets)) {
        if let Some(clip) = self.selected_visual() {
            let mut animation_presets = *clip.animation_presets();
            change(&mut animation_presets);
            let edit = Edit::AnimationPresets {
                clip: clip.id(),
                animation_presets,
            };
            self.preview_edit(&edit);
        }
        skip_view();
    }

    pub(crate) fn change_animation_presets(&mut self, change: impl FnOnce(&mut AnimationPresets)) {
        match self.selected_visual().map(|clip| clip.id()) {
            Some(clip) => self.animation_preset_on(clip, change),
            None => skip_view(),
        }
    }

    pub(crate) fn ease_animation_preset(&mut self, clip: ClipId, slot: PresetSlot, easing: Easing) {
        self.animation_preset_on(clip, |animation_presets| {
            slot.tune(animation_presets, |part| *part.easing() = easing)
        });
    }

    pub(crate) fn open_animation_preset_easing(&mut self, slot: PresetSlot, anchor: Anchor) {
        match self.selected_visual().map(|clip| clip.id()) {
            Some(clip) => {
                self.session.popup =
                    Some(Popup::Easing(Picking::animation_preset(clip, slot, anchor)))
            }
            None => skip_view(),
        }
    }

    fn animation_preset_on(&mut self, id: ClipId, change: impl FnOnce(&mut AnimationPresets)) {
        self.live.settle();
        let Some(clip) = self
            .project()
            .locate(id)
            .map(|(_, clip)| clip)
            .filter(|clip| self.project().is_visual(clip))
        else {
            skip_view();
            return;
        };
        let mut animation_presets = *clip.animation_presets();
        change(&mut animation_presets);
        if animation_presets == *clip.animation_presets() {
            skip_view();
            return;
        }
        self.edit(Edit::AnimationPresets {
            clip: id,
            animation_presets,
        });
    }
}
