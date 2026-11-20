use animate_body::Worlds;
use animate_solve::Handle;
use rikka_model::glam::Vec3;

use crate::easy::scene::Scene;

const HAND_GAP: f32 = 0.03;

#[derive(Debug)]
pub(super) struct Clearance {
    worlds: Worlds,
    gap: f32,
}

impl Clearance {
    pub(super) fn new(scene: &Scene, handle: Handle, point: Vec3) -> Option<Self> {
        let Handle::Hand(_) = handle else {
            return None;
        };
        let gap = HAND_GAP * scene.body.scale();
        let outside = !scene.body.bulk().holds(&scene.shown, point, gap);
        outside.then(|| Self {
            worlds: scene.shown.clone(),
            gap,
        })
    }

    pub(super) fn lift(&self, scene: &Scene, target: Vec3) -> Vec3 {
        let toward = -scene.lens.ahead(target);
        scene
            .body
            .bulk()
            .surface(&self.worlds, target, toward, self.gap)
    }
}
