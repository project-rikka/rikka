use glam::Vec3;
use rikka_physics_tailor::ClothAsset;

use crate::support::miku_asset;

const RADIUS: f32 = 0.06;

fn hem(asset: &ClothAsset) -> (usize, usize) {
    asset
        .pieces
        .iter()
        .enumerate()
        .flat_map(|(piece, piece_data)| {
            piece_data
                .particles
                .iter()
                .enumerate()
                .filter(|(_, particle)| !particle.pinned)
                .map(move |(particle, data)| (piece, particle, data.reach))
        })
        .max_by(|one, two| one.2.total_cmp(&two.2))
        .map(|(piece, particle, _)| (piece, particle))
        .expect("the sample wears loose cloth")
}

#[test]
fn pick_selects_nearest_free_particle_and_neighbours() {
    let asset = miku_asset();
    let (piece, particle) = hem(asset);
    let at = asset.pieces[piece].particles[particle].rest;
    let pick = asset.pick(at, RADIUS).expect("cloth lies under the point");
    assert_eq!((pick.piece, pick.centre), (piece, particle));
    let particles = &asset.pieces[piece].particles;
    assert!(
        pick.members
            .iter()
            .any(|member| member.particle == particle && member.weight == 1.0)
    );
    assert!(
        pick.members.len() > 1,
        "a hand holds more than one particle"
    );
    for member in &pick.members {
        assert!(!particles[member.particle].pinned);
        assert!(member.weight > 0.0 && member.weight <= 1.0);
        assert!(particles[member.particle].rest.distance(at) <= RADIUS);
    }
}

#[test]
fn pick_requires_cloth_within_radius() {
    let asset = miku_asset();
    let far = Vec3::new(0.0, 100.0, 0.0);
    assert_eq!(asset.pick(far, RADIUS), None);
    assert_eq!(asset.pick(Vec3::ZERO, 0.0), None);
}
