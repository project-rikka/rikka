use super::RigidWorld;
use crate::body::NONE;
use crate::island::IslandList;
use crate::set::{AWAKE_SET, swap_remove};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Member {
    Body,
    Contact,
    Joint,
}

impl RigidWorld {
    fn links(&mut self, member: Member, id: u32) -> (&mut u32, &mut u32, &mut u32) {
        match member {
            Member::Body => {
                let r = self.bodies.get_mut(id);
                (&mut r.island, &mut r.island_prev, &mut r.island_next)
            }
            Member::Contact => {
                let r = self.contacts.get_mut(id);
                (&mut r.island, &mut r.island_prev, &mut r.island_next)
            }
            Member::Joint => {
                let r = self.joints.get_mut(id);
                (&mut r.island, &mut r.island_prev, &mut r.island_next)
            }
        }
    }

    fn list(&mut self, island: u32, member: Member) -> &mut IslandList {
        let record = self.islands.get_mut(island);
        match member {
            Member::Body => &mut record.bodies,
            Member::Contact => &mut record.contacts,
            Member::Joint => &mut record.joints,
        }
    }

    fn append(&mut self, island: u32, member: Member, id: u32) {
        let tail = self.list(island, member).tail;
        {
            let (owner, prev, next) = self.links(member, id);
            *owner = island;
            *prev = tail;
            *next = NONE;
        }
        if tail != NONE {
            *self.links(member, tail).2 = id;
        }
        let list = self.list(island, member);
        if list.head == NONE {
            list.head = id;
        }
        list.tail = id;
        list.count += 1;
        self.islands.get_mut(island).dirty = true;
    }

    fn detach(&mut self, member: Member, id: u32) -> u32 {
        let (island, prev, next) = {
            let (owner, prev, next) = self.links(member, id);
            let result = (*owner, *prev, *next);
            *owner = NONE;
            *prev = NONE;
            *next = NONE;
            result
        };
        if prev != NONE {
            *self.links(member, prev).2 = next;
        }
        if next != NONE {
            *self.links(member, next).1 = prev;
        }
        let list = self.list(island, member);
        if list.head == id {
            list.head = next;
        }
        if list.tail == id {
            list.tail = prev;
        }
        list.count -= 1;
        self.islands.get_mut(island).dirty = true;
        island
    }

    pub(crate) fn create_island_for(&mut self, body: u32) {
        let set = self.bodies.get(body).set;
        let local = self.sets.get(set).islands.len() as u32;
        let island = self.islands.allocate(set, local);
        self.sets.get_mut(set).islands.push(island);
        self.append(island, Member::Body, body);
    }

    fn destroy_island(&mut self, island: u32) {
        let record = *self.islands.get(island);
        if let Some(moved) = swap_remove(&mut self.sets.get_mut(record.set).islands, record.local) {
            self.islands.get_mut(moved).local = record.local;
        }
        self.islands.free(island);
    }

    pub(crate) fn remove_body_from_island(&mut self, body: u32) {
        if self.bodies.get(body).island == NONE {
            return;
        }
        let island = self.detach(Member::Body, body);
        let record = self.islands.get_mut(island);
        record.constraint_removed += 1;
        if record.bodies.count == 0 {
            self.destroy_island(island);
        }
    }

    fn constraint_island(&self, a: u32, b: u32) -> (u32, u32) {
        (self.bodies.get(a).island, self.bodies.get(b).island)
    }

    fn merge(&mut self, a: u32, b: u32) -> u32 {
        let (count_a, count_b) = (
            self.islands.get(a).bodies.count,
            self.islands.get(b).bodies.count,
        );
        let (keep, gone) = if count_a > count_b || (count_a == count_b && a < b) {
            (a, b)
        } else {
            (b, a)
        };
        for member in [Member::Body, Member::Contact, Member::Joint] {
            let mut id = self.list(gone, member).head;
            while id != NONE {
                let next = *self.links(member, id).2;
                self.detach(member, id);
                self.append(keep, member, id);
                id = next;
            }
        }
        let removed = self.islands.get(gone).constraint_removed;
        self.islands.get_mut(keep).constraint_removed += removed;
        self.destroy_island(gone);
        keep
    }

    fn link(&mut self, member: Member, id: u32, a: u32, b: u32) {
        let target = match self.constraint_island(a, b) {
            (NONE, NONE) => return,
            (x, NONE) | (NONE, x) => x,
            (x, y) if x == y => x,
            (x, y) => self.merge(x, y),
        };
        self.append(target, member, id);
    }

    pub(crate) fn link_contact(&mut self, contact: u32) {
        let record = *self.contacts.get(contact);
        self.link(Member::Contact, contact, record.body_a(), record.body_b());
    }

    pub(crate) fn unlink_contact(&mut self, contact: u32) {
        let island = self.detach(Member::Contact, contact);
        self.islands.get_mut(island).constraint_removed += 1;
    }

    pub(crate) fn link_joint(&mut self, joint: u32) {
        let record = *self.joints.get(joint);
        self.link(Member::Joint, joint, record.body_a, record.body_b);
    }

    pub(crate) fn unlink_joint(&mut self, joint: u32) {
        let island = self.detach(Member::Joint, joint);
        self.islands.get_mut(island).constraint_removed += 1;
    }

    pub(crate) fn split_island(&mut self, island: u32) {
        let set = self.islands.get(island).set;
        debug_assert_eq!(set, AWAKE_SET);
        let mut bodies = std::mem::take(&mut self.solver.split_bodies);
        let mut contacts = std::mem::take(&mut self.solver.split_contacts);
        let mut joints = std::mem::take(&mut self.solver.split_joints);
        let mut stack = std::mem::take(&mut self.solver.split_stack);
        bodies.clear();
        contacts.clear();
        joints.clear();
        for (member, out) in [
            (Member::Body, &mut bodies),
            (Member::Contact, &mut contacts),
            (Member::Joint, &mut joints),
        ] {
            let mut id = self.list(island, member).head;
            while id != NONE {
                out.push(id);
                id = *self.links(member, id).2;
            }
        }
        for &body in &bodies {
            self.detach(Member::Body, body);
        }
        for &contact in &contacts {
            self.detach(Member::Contact, contact);
        }
        for &joint in &joints {
            self.detach(Member::Joint, joint);
        }
        self.islands.get_mut(island).constraint_removed = 0;
        let mut current = island;
        let mut first = true;
        for &seed in &bodies {
            if self.bodies.get(seed).island != NONE {
                continue;
            }
            if !first {
                let local = self.sets.get(set).islands.len() as u32;
                current = self.islands.allocate(set, local);
                self.sets.get_mut(set).islands.push(current);
            }
            first = false;
            stack.clear();
            stack.push(seed);
            self.append(current, Member::Body, seed);
            while let Some(body) = stack.pop() {
                let mut key = self.bodies.get(body).contact_head;
                while key != NONE {
                    let contact = *self.contacts.get(key >> 1);
                    key = contact.edges[(key & 1) as usize].next;
                    if contact.color == NONE {
                        continue;
                    }
                    let other = contact.edges[0].body ^ contact.edges[1].body ^ body;
                    if self.pending_member(other) {
                        self.append(current, Member::Body, other);
                        stack.push(other);
                    }
                }
                let mut key = self.bodies.get(body).joint_head;
                while key != NONE {
                    let joint = *self.joints.get(key >> 1);
                    key = joint.edges[(key & 1) as usize].next;
                    if joint.set != AWAKE_SET {
                        continue;
                    }
                    let other = joint.body_a ^ joint.body_b ^ body;
                    if self.pending_member(other) {
                        self.append(current, Member::Body, other);
                        stack.push(other);
                    }
                }
            }
        }
        for &contact in &contacts {
            let record = *self.contacts.get(contact);
            self.link(Member::Contact, contact, record.body_a(), record.body_b());
        }
        for &joint in &joints {
            let record = *self.joints.get(joint);
            self.link(Member::Joint, joint, record.body_a, record.body_b);
        }
        self.solver.split_bodies = bodies;
        self.solver.split_contacts = contacts;
        self.solver.split_joints = joints;
        self.solver.split_stack = stack;
    }

    fn pending_member(&self, body: u32) -> bool {
        let record = self.bodies.get(body);
        record.kind == crate::BodyKind::Dynamic && record.island == NONE && record.set == AWAKE_SET
    }
}
