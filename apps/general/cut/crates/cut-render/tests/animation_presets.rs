use crate::support::{Studio, apply, engine, insert, is_near, pixel_at, pixels, request, square};
use cut_project::{
    AnimationPreset, AnimationPresetKind, AnimationPresets, Art, ClipId, Edit, Graphic, Placement,
    Project,
};
use rikka_figure::Lettering;
use rikka_types::Color;

const RED: [u8; 4] = [255, 0, 0, 255];

fn entering(project: &Project, clip: ClipId, animation_preset: AnimationPreset) -> Project {
    apply(
        project,
        Edit::AnimationPresets {
            clip,
            animation_presets: AnimationPresets {
                in_preset: Some(animation_preset),
                ..AnimationPresets::default()
            },
        },
    )
}

fn red_square(project: &Project, animation_preset: AnimationPreset) -> Project {
    let (project, clip) = insert(project, square(400.0, Color::rgb(1.0, 0.0, 0.0)), 0);
    entering(&project, clip, animation_preset)
}

fn words(size: f32, placement: Placement) -> Graphic {
    Graphic::new(Art::text(Lettering {
        text: "AB".into(),
        size,
        weight: Lettering::BOLD,
        color: Color::rgb(1.0, 0.0, 0.0),
        ..Lettering::default()
    }))
    .with_placement(placement)
}

fn caption(project: &Project, animation_preset: AnimationPreset) -> Project {
    let (project, clip) = insert(project, words(200.0, Placement::default()), 0);
    entering(&project, clip, animation_preset)
}

fn inked(rgba: &[u8], columns: std::ops::Range<u32>) -> usize {
    columns
        .flat_map(|x| (0..540).map(move |y| (x, y)))
        .filter(|(x, y)| pixel_at(rgba, 960, *x, *y)[0] > 128)
        .count()
}

#[test]
fn fade_in_is_half_opaque_halfway() {
    let engine = engine();
    let mut studio = Studio::new(&engine);
    let project = red_square(
        &Project::new(),
        AnimationPreset {
            length: 10,
            ..AnimationPreset::new_in(AnimationPresetKind::Fade)
        },
    );

    let halfway = pixels(&studio.compose(&request(project.clone(), 5, 0.25)).picture);
    assert!(is_near(
        pixel_at(&halfway, 480, 240, 135),
        [128, 0, 0, 255],
        3
    ));
    let final_pixels = pixels(&studio.compose(&request(project, 10, 0.25)).picture);
    assert!(is_near(pixel_at(&final_pixels, 480, 240, 135), RED, 2));
}

#[test]
fn typewriter_reveals_text_by_letter() {
    let engine = engine();
    let mut studio = Studio::new(&engine);
    let project = caption(
        &Project::new(),
        AnimationPreset {
            length: 10,
            ..AnimationPreset::new_in(AnimationPresetKind::Typewriter)
        },
    );

    let halfway = pixels(&studio.compose(&request(project.clone(), 5, 0.5)).picture);
    assert!(inked(&halfway, 380..470) > 200);
    assert!(inked(&halfway, 490..600) == 0);
    let typed = pixels(&studio.compose(&request(project, 10, 0.5)).picture);
    assert!(inked(&typed, 490..600) > 200);
}
