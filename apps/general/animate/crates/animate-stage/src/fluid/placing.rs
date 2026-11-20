use animate_cast::Shot;
use rikka_model::Bounds;
use rikka_model::glam::Vec3;

const CHEST: f32 = 0.6;
const BEFORE: f32 = 0.1;
const AHEAD: f32 = 0.6;
const LOWEST_LAUNCH: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placing {
    pub launch: Vec3,
    pub target: Vec3,
}

impl Placing {
    pub fn for_figure(figure: Bounds) -> Self {
        let centre = figure.center();
        let height = figure.max.y - figure.min.y;
        let front = figure.max.z + BEFORE;
        Self {
            launch: Vec3::new(centre.x, figure.min.y + height * CHEST, front),
            target: Vec3::new(centre.x, 0.0, front + AHEAD),
        }
    }

    pub fn for_view(shot: &Shot) -> Self {
        let looked = shot.target();
        let ahead = looked - shot.eye();
        let ahead = Vec3::new(ahead.x, 0.0, ahead.z).normalize_or(Vec3::NEG_Z);
        let launch = Vec3::new(looked.x, looked.y.max(LOWEST_LAUNCH), looked.z);
        Self {
            launch,
            target: Vec3::new(launch.x, 0.0, launch.z) + ahead * AHEAD,
        }
    }
}
