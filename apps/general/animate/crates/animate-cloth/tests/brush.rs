use animate_cloth::{Brush, Dab, Mirror, Surface, Tool};
use rikka_humanoid::Anatomy;
use rikka_model_core::glam::Vec3;
use rikka_physics_tailor::{ClothAnalysis, ClothSettings, Role};

use crate::support::miku;

#[test]
fn mirrored_stroke_paints_both_sides() {
    let model = miku();
    let surface = Surface::rest(&model);
    let mirror = Mirror::from_surface(&surface);
    let hit = surface
        .cast(Vec3::new(0.12, 1.25, 3.0), Vec3::NEG_Z)
        .expect("the left of the chest is in reach");
    let brush = Brush {
        tool: Tool::Free,
        ..Brush::default()
    };
    let mut cloth_settings = ClothSettings::new();
    let painted = brush.dab(
        &mut cloth_settings,
        &surface,
        Some(&mirror),
        Dab {
            centre: hit.point,
            radius: 0.04,
            erase: false,
        },
    );
    assert!(painted > 0);
    let anatomy = Anatomy::recognize(&model);
    let analysis =
        ClothAnalysis::new(&model, &anatomy, &cloth_settings).expect("the sample is analyzed");
    let plain = ClothAnalysis::new(&model, &anatomy, &ClothSettings::new())
        .expect("the sample is analyzed");
    let (mut pairs, mut changed) = (0, 0);
    for index in 0..surface.len() {
        let key = surface.keys()[index];
        if cloth_settings.paint_at(key).role.is_none() {
            continue;
        }
        assert_eq!(analysis.role(key), Some(Role::Free));
        if plain.role(key) != Some(Role::Free) {
            changed += 1;
        }
        let twin = mirror
            .twin(index)
            .filter(|_| surface.points()[index].x > 0.0);
        let Some(twin) = twin else {
            continue;
        };
        let twin_key = surface.keys()[twin];
        assert_eq!(cloth_settings.paint_at(twin_key).role, Some(Role::Free));
        assert_eq!(analysis.role(key), analysis.role(twin_key));
        pairs += 1;
    }
    assert!(changed > 0, "the stroke frees what was held");
    assert!(pairs > 0, "the stroke reached the figure's left");
    assert!(
        surface
            .keys()
            .iter()
            .zip(surface.points())
            .any(|(key, point)| point.x < 0.0 && cloth_settings.paint_at(*key).role.is_some()),
        "and its mirror reached the right"
    );

    let erased = brush.dab(
        &mut cloth_settings,
        &surface,
        Some(&mirror),
        Dab {
            centre: hit.point,
            radius: 0.04,
            erase: true,
        },
    );
    assert_eq!(erased, painted);
    assert_eq!(cloth_settings.paint_entries().count(), 0);
}
