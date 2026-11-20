use rikka_physics_math::{Quat, Vec3};

use super::RigidWorld;
use crate::body::NONE;
use crate::set::{AWAKE_SET, DISABLED_SET, FIRST_SLEEPING_SET, swap_remove};

impl RigidWorld {
    fn move_contact_list(&mut self, contact: u32, target: u32) {
        self.remove_contact_from_set(contact);
        let list = if target == AWAKE_SET {
            &mut self.awake_contacts
        } else {
            &mut self.sets.get_mut(target).contacts
        };
        list.push(contact);
        let index = list.len() as u32 - 1;
        let record = self.contacts.get_mut(contact);
        record.set = target;
        record.set_index = index;
    }

    pub(crate) fn try_sleep_island(&mut self, island: u32) {
        let record = *self.islands.get(island);
        debug_assert_eq!(record.set, AWAKE_SET);
        if record.constraint_removed > 0 && record.bodies.count > 1 {
            return;
        }
        let sleep = self.sets.allocate_sleeping();
        let mut body = record.bodies.head;
        while body != NONE {
            let body_record = *self.bodies.get(body);
            let next = body_record.island_next;
            if body_record.move_event != NONE
                && let Some(event) = self
                    .events
                    .body_moves
                    .get_mut(body_record.move_event as usize)
            {
                event.fell_asleep = true;
            }
            let local = body_record.local as usize;
            let awake = self.sets.awake();
            let pose = awake.poses[local];
            let motion = awake.motion[local];
            let mut state = awake.states[local];
            state.v = Vec3::ZERO;
            state.w = Vec3::ZERO;
            let new_local = self
                .sets
                .get_mut(sleep)
                .push_body(body, pose, motion, state);
            self.remove_from_set(body, AWAKE_SET, body_record.local);
            let moved = self.bodies.get_mut(body);
            moved.set = sleep;
            moved.local = new_local;
            moved.move_event = NONE;
            self.refresh_solver_indices(body);

            let mut key = self.bodies.get(body).contact_head;
            while key != NONE {
                let contact = *self.contacts.get(key >> 1);
                let id = key >> 1;
                key = contact.edges[(key & 1) as usize].next;
                if contact.color != NONE || contact.set != AWAKE_SET {
                    continue;
                }
                let other = contact.edges[0].body ^ contact.edges[1].body ^ body;
                if self.bodies.get(other).set == AWAKE_SET {
                    continue;
                }
                self.move_contact_list(id, DISABLED_SET);
            }
            body = next;
        }

        let mut contact = record.contacts.head;
        while contact != NONE {
            let next = self.contacts.get(contact).island_next;
            if self.contacts.get(contact).color != NONE {
                self.remove_contact_from_graph(contact);
            }
            self.move_contact_list(contact, sleep);
            contact = next;
        }

        let mut joint = record.joints.head;
        while joint != NONE {
            let next = self.joints.get(joint).island_next;
            if self.joints.get(joint).color != NONE {
                self.remove_joint_from_graph(joint);
            }
            let list = &mut self.sets.get_mut(sleep).joints;
            list.push(joint);
            let index = list.len() as u32 - 1;
            let joint_record = self.joints.get_mut(joint);
            joint_record.set = sleep;
            joint_record.local = index;
            joint = next;
        }

        if let Some(moved) = swap_remove(&mut self.sets.awake_mut().islands, record.local) {
            self.islands.get_mut(moved).local = record.local;
        }
        let islands = &mut self.sets.get_mut(sleep).islands;
        islands.push(island);
        let local = islands.len() as u32 - 1;
        let island_record = self.islands.get_mut(island);
        island_record.set = sleep;
        island_record.local = local;
    }

    pub(crate) fn wake_set(&mut self, set: u32) {
        debug_assert!(set >= FIRST_SLEEPING_SET);
        let bodies = std::mem::take(&mut self.sets.get_mut(set).bodies);
        for (local, &body) in bodies.iter().enumerate() {
            let sleeping = self.sets.get(set);
            let pose = sleeping.poses[local];
            let motion = sleeping.motion[local];
            let mut state = sleeping.states[local];
            state.dp = Vec3::ZERO;
            state.dq = Quat::IDENTITY;
            let new_local = self.sets.awake_mut().push_body(body, pose, motion, state);
            let record = self.bodies.get_mut(body);
            record.set = AWAKE_SET;
            record.local = new_local;
            record.sleep_time = 0.0;
        }
        for &body in &bodies {
            let mut key = self.bodies.get(body).contact_head;
            while key != NONE {
                let id = key >> 1;
                let contact = *self.contacts.get(id);
                key = contact.edges[(key & 1) as usize].next;
                if contact.set == DISABLED_SET {
                    self.move_contact_list(id, AWAKE_SET);
                }
            }
            self.refresh_solver_indices(body);
        }
        let contacts = std::mem::take(&mut self.sets.get_mut(set).contacts);
        for &contact in &contacts {
            self.awake_contacts.push(contact);
            let index = self.awake_contacts.len() as u32 - 1;
            let record = self.contacts.get_mut(contact);
            record.set = AWAKE_SET;
            record.set_index = index;
            self.add_contact_to_graph(contact);
        }
        let joints = std::mem::take(&mut self.sets.get_mut(set).joints);
        for &joint in &joints {
            self.joints.get_mut(joint).set = AWAKE_SET;
            self.add_joint_to_graph(joint);
        }
        let islands = std::mem::take(&mut self.sets.get_mut(set).islands);
        for &island in &islands {
            let awake = self.sets.awake_mut();
            awake.islands.push(island);
            let local = awake.islands.len() as u32 - 1;
            let record = self.islands.get_mut(island);
            record.set = AWAKE_SET;
            record.local = local;
            record.dirty = true;
        }
        self.sets.release(set);
    }
}
