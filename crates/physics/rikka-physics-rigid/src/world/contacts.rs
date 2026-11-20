use rikka_physics_collision::{
    DISJOINT, STARTED_TOUCHING, STATE_CHANGES, STOPPED_TOUCHING, TOUCHING,
};
use rikka_physics_core::prefetch;

use super::RigidWorld;
use crate::body::{BodyKind, NONE};
use crate::contact::{ContactRecord, EVENTS, HIT_EVENTS};
use crate::graph::Constraint;
use crate::set::{AWAKE_SET, FIRST_SLEEPING_SET, swap_remove};
use crate::{ContactBeginEvent, ContactEndEvent};

const CHANGE_PREFETCH: usize = 4;

impl RigidWorld {
    fn set_edge_prev(&mut self, key: u32, prev: u32) {
        self.contacts.get_mut(key >> 1).edges[(key & 1) as usize].prev = prev;
    }

    fn set_edge_next(&mut self, key: u32, next: u32) {
        self.contacts.get_mut(key >> 1).edges[(key & 1) as usize].next = next;
    }

    pub(crate) fn destroy_contact(&mut self, id: u32, wake: bool) {
        let record = *self.contacts.get(id);
        if record.flags & HIT_EVENTS != 0 {
            self.hit_contacts -= 1;
        }
        let touching = self.headers[id as usize].flags & TOUCHING != 0;
        if touching && record.flags & EVENTS != 0 {
            self.events.contact_end.push(ContactEndEvent {
                shape_a: self.shape_id(record.shape_a),
                shape_b: self.shape_id(record.shape_b),
            });
        }
        if record.color != NONE {
            self.remove_contact_from_graph(id);
        }
        if record.island != NONE {
            self.unlink_contact(id);
        }
        self.remove_contact_from_set(id);
        for (side, edge) in record.edges.into_iter().enumerate() {
            let key = id << 1 | side as u32;
            if edge.prev != NONE {
                self.set_edge_next(edge.prev, edge.next);
            } else {
                self.bodies.get_mut(edge.body).contact_head = edge.next;
            }
            if edge.next != NONE {
                self.set_edge_prev(edge.next, edge.prev);
            }
            debug_assert!(edge.prev != key);
            self.bodies.get_mut(edge.body).contact_count -= 1;
        }
        self.broadphase.remove_pair(record.shape_a, record.shape_b);
        self.contacts.remove(id);
        if wake {
            for edge in record.edges {
                self.wake_body(edge.body);
            }
        }
    }

    pub(crate) fn remove_contact_from_set(&mut self, id: u32) {
        let record = *self.contacts.get(id);
        let list = if record.set == AWAKE_SET {
            &mut self.awake_contacts
        } else {
            &mut self.sets.get_mut(record.set).contacts
        };
        if let Some(moved) = swap_remove(list, record.set_index) {
            self.contacts.get_mut(moved).set_index = record.set_index;
        }
    }

    pub(crate) fn add_contact_to_graph(&mut self, id: u32) {
        let record = *self.contacts.get(id);
        let a = (
            record.body_a(),
            self.bodies.get(record.body_a()).kind == BodyKind::Dynamic,
        );
        let b = (
            record.body_b(),
            self.bodies.get(record.body_b()).kind == BodyKind::Dynamic,
        );
        let (color, index) = self.graph.add(Constraint::Contact, id, a, b);
        let record = self.contacts.get_mut(id);
        record.color = color;
        record.color_index = index;
    }

    pub(crate) fn remove_contact_from_graph(&mut self, id: u32) {
        let record = *self.contacts.get(id);
        if let Some(moved) = self.graph.remove(
            Constraint::Contact,
            record.color,
            record.color_index,
            record.body_a(),
            record.body_b(),
        ) {
            self.contacts.get_mut(moved).color_index = record.color_index;
        }
        let record = self.contacts.get_mut(id);
        record.color = NONE;
        record.color_index = NONE;
    }

    pub(crate) fn process_contact_changes(&mut self, changes: &[u32]) {
        for (index, &id) in changes.iter().enumerate() {
            if let Some(&far) = changes.get(index + 2 * CHANGE_PREFETCH) {
                prefetch(self.contacts.get(far), size_of::<ContactRecord>());
                prefetch(&self.headers[far as usize], 64);
            }
            if let Some(&near) = changes.get(index + CHANGE_PREFETCH) {
                let record = self.contacts.get(near);
                for edge in &record.edges {
                    prefetch(self.bodies.get(edge.body), 64);
                }
                if record.island != NONE {
                    prefetch(self.islands.get(record.island), 64);
                }
            }
            let flags = self.headers[id as usize].flags;
            self.headers[id as usize].flags &= !STATE_CHANGES;
            let record = *self.contacts.get(id);
            let events = record.flags & EVENTS != 0;
            if flags & DISJOINT != 0 {
                if flags & STOPPED_TOUCHING != 0 {
                    self.headers[id as usize].flags |= TOUCHING;
                }
                self.destroy_contact(id, false);
            } else if flags & STARTED_TOUCHING != 0 {
                for body in [record.body_a(), record.body_b()] {
                    let set = self.bodies.get(body).set;
                    if set >= FIRST_SLEEPING_SET {
                        self.wake_set(set);
                    }
                }
                self.link_contact(id);
                self.add_contact_to_graph(id);
                if events {
                    self.events.contact_begin.push(ContactBeginEvent {
                        shape_a: self.shape_id(record.shape_a),
                        shape_b: self.shape_id(record.shape_b),
                    });
                }
            } else if flags & STOPPED_TOUCHING != 0 {
                if record.color != NONE {
                    self.remove_contact_from_graph(id);
                }
                if record.island != NONE {
                    self.unlink_contact(id);
                }
                if events {
                    self.events.contact_end.push(ContactEndEvent {
                        shape_a: self.shape_id(record.shape_a),
                        shape_b: self.shape_id(record.shape_b),
                    });
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContactData {
    pub shape_a: crate::ShapeId,
    pub shape_b: crate::ShapeId,
    pub manifolds: Vec<rikka_physics_collision::ContactManifold>,
}

impl RigidWorld {
    pub fn body_contacts(&self, id: crate::BodyId) -> crate::Result<Vec<ContactData>> {
        let body = self.body_index(id)?;
        let mut out = Vec::new();
        let mut key = self.bodies.get(body).contact_head;
        while key != NONE {
            let contact = key >> 1;
            let record = self.contacts.get(contact);
            key = record.edges[(key & 1) as usize].next;
            let header = &self.headers[contact as usize];
            if header.flags & TOUCHING == 0 {
                continue;
            }
            let mut manifolds = vec![self.geometry[contact as usize].manifold];
            self.narrowphase
                .pools()
                .extend_extras(header.extra, &mut manifolds);
            out.push(ContactData {
                shape_a: self.shape_id(record.shape_a),
                shape_b: self.shape_id(record.shape_b),
                manifolds,
            });
        }
        Ok(out)
    }
}
