use rikka_physics_geometry::Shape;

use crate::{Filter, Material};

#[derive(Clone, Debug)]
pub struct Collider {
    pub shape: Shape,
    pub materials: Vec<Material>,
    pub filter: Filter,
    pub sensor: bool,
    pub owner: u32,
    pub contact_events: bool,
    pub hit_events: bool,
    pub sensor_events: bool,
    pub contact_response: bool,
}

impl Collider {
    #[inline]
    pub fn responds(&self) -> bool {
        !self.sensor && self.contact_response
    }

    #[inline]
    pub fn material(&self, index: u32) -> &Material {
        self.materials
            .get(index as usize)
            .unwrap_or(&self.materials[0])
    }
}
