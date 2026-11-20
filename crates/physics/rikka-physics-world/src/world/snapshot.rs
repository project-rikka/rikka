use super::World;
use crate::snapshot::Snapshot;

impl World {
    pub fn snapshot(&self) -> Snapshot {
        let mut snapshot = Snapshot::default();
        self.capture(&mut snapshot);
        snapshot
    }

    pub fn capture(&self, snapshot: &mut Snapshot) {
        snapshot.time = self.clock.time();
        snapshot.state_hash = self.state_hash();
        snapshot.bodies.clear();
        snapshot.bodies.extend(self.bodies.iter().map(|&body| {
            let pose = self.pose(body).expect("world body is alive");
            (body, pose)
        }));
        snapshot.contacts.clone_from(&self.events.contacts);
        snapshot.joints.clone_from(&self.events.joints);
    }
}
