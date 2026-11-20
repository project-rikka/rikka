use rikka_physics_math::Transform;
use rikka_physics_rigid::{BodyId, JointEvent};

use crate::ContactEvent;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub(crate) time: f64,
    pub(crate) state_hash: u64,
    pub(crate) bodies: Vec<(BodyId, Transform)>,
    pub(crate) contacts: Vec<ContactEvent>,
    pub(crate) joints: Vec<JointEvent>,
}

impl Snapshot {
    pub fn time(&self) -> f64 {
        self.time
    }

    pub fn state_hash(&self) -> u64 {
        self.state_hash
    }

    pub fn pose(&self, body: BodyId) -> Option<Transform> {
        self.bodies
            .binary_search_by_key(&body.index, |(id, _)| id.index)
            .ok()
            .map(|at| self.bodies[at])
            .filter(|(id, _)| *id == body)
            .map(|(_, pose)| pose)
    }

    pub fn bodies(&self) -> &[(BodyId, Transform)] {
        &self.bodies
    }

    pub fn contact_events(&self) -> &[ContactEvent] {
        &self.contacts
    }

    pub fn joint_events(&self) -> &[JointEvent] {
        &self.joints
    }
}
