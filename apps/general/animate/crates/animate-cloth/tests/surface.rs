use std::sync::Arc;

use animate_cloth::{Mirror, Surface};
use rikka_model_core::Instance;
use rikka_model_core::glam::Vec3;

use crate::support::miku;

#[test]
fn chest_ray_hits_front_surface() {
    let model = miku();
    let surface = Surface::rest(&model);
    assert!(!surface.is_empty());
    let hit = surface
        .cast(Vec3::new(0.0, 1.2, 3.0), Vec3::NEG_Z)
        .expect("the ray meets the figure");
    assert!(hit.point.z > 0.0 && hit.point.z < 0.3, "{}", hit.point);
    assert!(surface.cast(Vec3::new(0.0, 1.2, 3.0), Vec3::Z).is_none());
}

#[test]
fn rest_pose_matches_rest_surface() {
    let model = Arc::new(miku());
    let mut instance = Instance::new(Arc::clone(&model));
    instance.refresh();
    let posed_surface = Surface::from_instance(&instance);
    let rest = Surface::rest(&model);
    assert_eq!(posed_surface.keys(), rest.keys());
    let worst = posed_surface
        .points()
        .iter()
        .zip(rest.points())
        .map(|(one, two)| one.distance(*two))
        .fold(0.0, f32::max);
    assert!(worst < 1e-3, "{worst}");
}

#[test]
fn mirror_pairs_are_symmetric() {
    let surface = Surface::rest(&miku());
    let mirror = Mirror::from_surface(&surface);
    let paired = (0..surface.len())
        .filter(|index| mirror.twin(*index).is_some())
        .count();
    assert!(
        paired * 10 >= surface.len() * 8,
        "{paired} of {} vertices",
        surface.len()
    );
    let lopsided = (0..surface.len())
        .filter_map(|index| Some((index, mirror.twin(index)?)))
        .filter(|(index, twin)| {
            let one = surface.points()[*index];
            let two = surface.points()[*twin];
            (one.x + two.x).abs() > 0.002
        })
        .count();
    assert_eq!(lopsided, 0);
}
