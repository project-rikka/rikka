use glam::{Quat, Vec3};
use rikka_physics_liquid::Throw;

const EARTH: Vec3 = Vec3::new(0.0, -9.81, 0.0);

fn apex(throw: &Throw, from: Vec3, gravity: Vec3) -> f32 {
    let up = -gravity.normalize();
    throw
        .path(from, gravity, 2001)
        .into_iter()
        .map(|point| point.dot(up))
        .fold(f32::MIN, f32::max)
}

#[test]
fn ballistic_throw_hits_target() {
    let gravities = [
        EARTH,
        Vec3::new(0.0, -1.62, 0.0),
        Quat::from_rotation_z(0.4) * EARTH,
        Quat::from_rotation_x(-0.9) * Vec3::new(0.0, -20.0, 0.0),
    ];
    let spots = [
        Vec3::ZERO,
        Vec3::new(0.0, 1.2, 0.0),
        Vec3::new(-0.4, 0.9, 0.3),
        Vec3::new(2.0, 0.1, -1.5),
        Vec3::new(0.0, 3.0, 0.0),
    ];
    for gravity in gravities {
        for from in spots {
            for to in spots {
                for arc in [0.0, 0.05, 0.4, 2.5] {
                    let Some(throw) = Throw::ballistic(from, to, gravity, arc) else {
                        let up = -gravity.normalize();
                        let across = (to - from) - up * (to - from).dot(up);
                        assert!(arc == 0.0 && (to - from).dot(up).abs() < 1e-5);
                        assert!(across.length() > 1e-5);
                        continue;
                    };
                    let landed = throw.at(from, gravity, throw.seconds);
                    assert!(
                        landed.distance(to) < 1e-3,
                        "{from} → {to}, arc {arc}, g {gravity}: {landed}"
                    );
                    let up = -gravity.normalize();
                    let highest = from.dot(up).max(to.dot(up)) + arc;
                    assert!(
                        (apex(&throw, from, gravity) - highest).abs() < 2e-3,
                        "{from} → {to}, arc {arc}"
                    );
                }
            }
        }
    }
}
