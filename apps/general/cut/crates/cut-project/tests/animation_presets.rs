use crate::support::{applied, clip, clips_on, inserted, near, near_point, square};
use cut_project::{
    AnimationPreset, AnimationPresetKind, AnimationPresets, Direction, Edge, Edit, LoopKind,
    LoopPreset, Project, Viewpoint,
};
use rikka_types::Point;

fn sliding_in() -> AnimationPreset {
    AnimationPreset {
        direction: Direction::Left,
        ..AnimationPreset::new_in(AnimationPresetKind::Slide)
    }
}

#[test]
fn in_animation_ends_at_rest() {
    let animation_presets = AnimationPresets {
        in_preset: Some(AnimationPreset {
            length: 10,
            ..sliding_in()
        }),
        ..AnimationPresets::default()
    };

    let first = animation_presets.transforms_at(0.0, 60, 0).whole;
    assert!(near_point(first.offset, Point::new(-200.0, 0.0), 1e-3));
    assert!(animation_presets.transforms_at(10.0, 60, 0).whole.is_rest());
    assert!(animation_presets.transforms_at(30.0, 60, 0).whole.is_rest());
    let halfway = animation_presets.transforms_at(5.0, 60, 0).whole.offset.x;
    assert!(halfway > -100.0 && halfway < 0.0);
}

#[test]
fn split_keeps_in_and_out_animations() {
    let (project, id) = inserted(&Project::new(), square(100.0), 0, 60);
    let animation_presets = AnimationPresets {
        in_preset: Some(AnimationPreset::new_in(AnimationPresetKind::Fade)),
        out_preset: Some(AnimationPreset::new_out(AnimationPresetKind::Fade)),
        loop_preset: Some(LoopPreset::new(LoopKind::Pulse)),
    };
    let project = applied(
        &project,
        Edit::AnimationPresets {
            clip: id,
            animation_presets,
        },
    );
    assert_eq!(*clip(&project, id).animation_presets(), animation_presets);

    let split = applied(
        &project,
        Edit::Split {
            clips: vec![id],
            at: 30,
        },
    );
    let halves = clips_on(&split, 0);
    assert!(
        halves[0].animation_presets().in_preset.is_some()
            && halves[0].animation_presets().out_preset.is_none()
    );
    assert!(
        halves[1].animation_presets().in_preset.is_none()
            && halves[1].animation_presets().out_preset.is_some()
    );

    let trimmed = applied(
        &project,
        Edit::Trim {
            clips: vec![id],
            edge: Edge::Head,
            by: 7,
        },
    );
    let pulse = |project: &Project, frame: i64| {
        project
            .shot(clip(project, id), frame, Viewpoint::Output)
            .expect("a shot")
            .visual
            .placement()
            .zoom
    };
    let phase = clip(&trimmed, id)
        .animation_presets()
        .loop_preset
        .map(|repeat| repeat.phase);
    assert_eq!(phase, Some(7));
    for frame in [7, 20, 33, 59] {
        assert!(near(pulse(&trimmed, frame), pulse(&project, frame), 1e-6));
    }
    let opacity = trimmed
        .shot(clip(&trimmed, id), 7, Viewpoint::Output)
        .map(|shot| shot.visual.placement().opacity);
    assert_eq!(opacity, Some(0.0));
}
