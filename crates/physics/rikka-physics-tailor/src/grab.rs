use std::sync::Arc;

use glam::Vec3;

use crate::pick::ParticlePick;

pub const MAX_GRABS: usize = 16;

#[derive(Clone, Debug)]
pub struct Grab {
    pub id: u64,
    pub pick: Arc<ParticlePick>,
    pub point: Vec3,
    pub strength: f32,
}
