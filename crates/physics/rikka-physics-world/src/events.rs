use rikka_physics_rigid::{
    ContactBeginEvent, ContactEndEvent, ContactHitEvent, JointEvent, RigidWorld, SensorBeginEvent,
    SensorEndEvent,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContactEvent {
    Begin(ContactBeginEvent),
    End(ContactEndEvent),
    Hit(ContactHitEvent),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct FrameEvents {
    pub contacts: Vec<ContactEvent>,
    pub joints: Vec<JointEvent>,
    pub sensor_begins: Vec<SensorBeginEvent>,
    pub sensor_ends: Vec<SensorEndEvent>,
}

impl FrameEvents {
    pub(crate) fn clear(&mut self) {
        self.contacts.clear();
        self.joints.clear();
        self.sensor_begins.clear();
        self.sensor_ends.clear();
    }

    pub(crate) fn collect(&mut self, rigid: &RigidWorld) {
        self.contacts.extend(
            rigid
                .contact_begin_events()
                .iter()
                .map(|&event| ContactEvent::Begin(event)),
        );
        self.contacts.extend(
            rigid
                .contact_end_events()
                .iter()
                .map(|&event| ContactEvent::End(event)),
        );
        self.contacts.extend(
            rigid
                .contact_hit_events()
                .iter()
                .map(|&event| ContactEvent::Hit(event)),
        );
        self.joints.extend_from_slice(rigid.joint_events());
        self.sensor_begins
            .extend_from_slice(rigid.sensor_begin_events());
        self.sensor_ends
            .extend_from_slice(rigid.sensor_end_events());
    }
}
