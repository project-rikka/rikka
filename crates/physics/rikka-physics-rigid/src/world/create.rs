use rikka_physics_collision::{ContactGeometry, ContactHeader, PairKind, pair_of};
use rikka_physics_core::{Block, Executor, Shared, Stage};

use super::RigidWorld;
use crate::body::NONE;
use crate::contact::{ContactRecord, EVENTS, HIT_EVENTS};
use crate::graph::Edge;
use crate::set::AWAKE_SET;

const CREATE_BLOCK: usize = 64;
const MOVE_BLOCK: usize = 2048;

const UNLINKED: Edge = Edge {
    body: NONE,
    prev: NONE,
    next: NONE,
};

const PENDING: ContactRecord = ContactRecord {
    shape_a: NONE,
    shape_b: NONE,
    edges: [UNLINKED; 2],
    set: AWAKE_SET,
    set_index: NONE,
    color: NONE,
    color_index: NONE,
    island: NONE,
    island_prev: NONE,
    island_next: NONE,
    solver_a: 0,
    solver_b: 0,
    flags: 0,
};

impl RigidWorld {
    pub(crate) fn create_contacts(&mut self, pairs: &[u64]) {
        if pairs.is_empty() {
            return;
        }
        let mut ids = std::mem::take(&mut self.solver.new_contacts);
        ids.clear();
        ids.extend(pairs.iter().map(|_| self.contacts.reserve(PENDING)));
        let slots = self.contacts.records.len();
        let filled = self.headers.len();
        reserve_on(&mut self.headers, slots, &*self.executor);
        reserve_on(&mut self.geometry, slots, &*self.executor);
        let colliders = &self.colliders;
        let bodies = &self.bodies;
        let recycle = self.config.recycling;
        let records = Shared::new(&mut self.contacts.records);
        let headers = unsafe { Shared::from_raw(self.headers.as_mut_ptr(), slots) };
        let geometry = unsafe { Shared::from_raw(self.geometry.as_mut_ptr(), slots) };
        let ids_ref = &ids;
        let task = |block: Block| {
            for index in block.range(CREATE_BLOCK, pairs.len()) {
                let (a, b) = pair_of(pairs[index]);
                let (kind, swap) =
                    PairKind::classify(&colliders[a as usize].shape, &colliders[b as usize].shape);
                let (shape_a, shape_b) = if swap { (b, a) } else { (a, b) };
                let collider_a = &colliders[shape_a as usize];
                let collider_b = &colliders[shape_b as usize];
                let (body_a, body_b) = (collider_a.owner, collider_b.owner);
                let mut flags = 0;
                if collider_a.contact_events && collider_b.contact_events {
                    flags |= EVENTS;
                }
                if collider_a.hit_events || collider_b.hit_events {
                    flags |= HIT_EVENTS;
                }
                let solver_index = |body: u32| {
                    let record = bodies.get(body);
                    if record.set == AWAKE_SET {
                        record.local + 1
                    } else {
                        0
                    }
                };
                let id = ids_ref[index] as usize;
                *unsafe { records.get(id) } = ContactRecord {
                    shape_a,
                    shape_b,
                    edges: [
                        Edge {
                            body: body_a,
                            ..UNLINKED
                        },
                        Edge {
                            body: body_b,
                            ..UNLINKED
                        },
                    ],
                    solver_a: solver_index(body_a),
                    solver_b: solver_index(body_b),
                    flags,
                    ..PENDING
                };
                let header = ContactHeader::new(shape_a, shape_b, body_a, body_b, kind, recycle);
                if id < filled {
                    *unsafe { headers.get(id) } = header;
                    *unsafe { geometry.get(id) } = ContactGeometry::new(kind);
                } else {
                    unsafe {
                        headers.as_ptr().add(id).write(header);
                        geometry.as_ptr().add(id).write(ContactGeometry::new(kind));
                    }
                }
            }
        };
        self.executor.run(
            &[Stage::for_items(
                "create contacts",
                pairs.len(),
                CREATE_BLOCK,
            )],
            &task,
        );
        unsafe {
            self.headers.set_len(slots);
            self.geometry.set_len(slots);
        }
        for &id in &ids {
            self.link_new_contact(id);
        }
        self.solver.new_contacts = ids;
    }

    fn link_new_contact(&mut self, id: u32) {
        let set_index = self.awake_contacts.len() as u32;
        self.awake_contacts.push(id);
        let record = self.contacts.get_mut(id);
        record.set_index = set_index;
        let (edges, flags) = (record.edges, record.flags);
        if flags & HIT_EVENTS != 0 {
            self.hit_contacts += 1;
        }
        for (side, edge) in edges.iter().enumerate() {
            let key = id << 1 | side as u32;
            let body = self.bodies.get_mut(edge.body);
            let head = body.contact_head;
            body.contact_head = key;
            body.contact_count += 1;
            self.contacts.get_mut(id).edges[side].next = head;
            if head != NONE {
                self.contacts.get_mut(head >> 1).edges[(head & 1) as usize].prev = key;
            }
        }
    }
}

fn reserve_on<T: Send>(items: &mut Vec<T>, len: usize, executor: &dyn Executor) {
    if items.capacity() >= len {
        return;
    }
    let count = items.len();
    let mut grown: Vec<T> = Vec::with_capacity(len.max(2 * items.capacity()));
    let from = unsafe { Shared::from_raw(items.as_mut_ptr(), count) };
    let to = unsafe { Shared::from_raw(grown.as_mut_ptr(), count) };
    executor.run(
        &[Stage::for_items("grow contacts", count, MOVE_BLOCK)],
        &|block: Block| {
            let range = block.range(MOVE_BLOCK, count);
            unsafe {
                std::ptr::copy_nonoverlapping(
                    from.as_ptr().add(range.start),
                    to.as_ptr().add(range.start),
                    range.len(),
                );
            }
        },
    );
    unsafe {
        items.set_len(0);
        grown.set_len(count);
    }
    *items = grown;
}
