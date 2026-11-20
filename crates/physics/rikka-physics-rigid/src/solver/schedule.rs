use std::collections::VecDeque;

use rikka_physics_collision::{ContactHeader, PairKind};
use rikka_physics_core::{Block, PerWorker, Shared, Stage};

use crate::body::{BodyRecord, NONE};
use crate::contact::{ContactRecord, SKIP};
use crate::island::Islands;
use crate::joint::{JointKind, JointRecord};
use crate::solver::contact::encode;
use crate::solver::lanes::MAX_LANES;
use crate::solver::plan::{Plan, TaskPlan, TaskStep};
use crate::table::Table;
use crate::world::RigidWorld;

const OVERFLOW_COLOR: u32 = 31;
const CONTACT_CLASS: u32 = u32::MAX;
const GROUP_LANES: u32 = 32;

#[derive(Clone, Copy, Debug)]
struct Entry {
    depth: u32,
    class: u32,
    id: u32,
    a: u32,
    b: u32,
}

#[derive(Clone, Copy, Debug)]
struct LocalBatch {
    class: u32,
    color: u32,
    count: u32,
    ids: [u32; MAX_LANES],
}

impl LocalBatch {
    fn new(class: u32, color: u32) -> Self {
        Self {
            class,
            color,
            count: 0,
            ids: [0; MAX_LANES],
        }
    }
}

#[derive(Debug, Default)]
struct IslandScratch {
    open: Vec<(u64, u32)>,
    queue: VecDeque<u32>,
    entries: Vec<Entry>,
    batches: Vec<LocalBatch>,
    joint_batches: Vec<LocalBatch>,
    offsets: Vec<u32>,
    neighbors: Vec<u32>,
}

#[derive(Debug)]
pub(crate) struct Scheduler {
    depth: Vec<u32>,
    used: Vec<u32>,
    last: Vec<u32>,
    order: Vec<u32>,
    stale: Vec<u32>,
    workers: PerWorker<IslandScratch>,
    merged: Vec<LocalBatch>,
    cache: Vec<Vec<LocalBatch>>,
    cached: Vec<bool>,
    volatile: Vec<bool>,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self {
            depth: Vec::new(),
            used: Vec::new(),
            last: Vec::new(),
            order: Vec::new(),
            stale: Vec::new(),
            workers: PerWorker::new(1, |_| IslandScratch::default()),
            merged: Vec::new(),
            cache: Vec::new(),
            cached: Vec::new(),
            volatile: Vec::new(),
        }
    }
}

fn class_of(kind: JointKind, layout: u32) -> u32 {
    (kind.index() as u32) << 16 | layout
}

struct IslandContext<'a> {
    bodies: &'a Table<BodyRecord>,
    contacts: &'a Table<ContactRecord>,
    joints: &'a Table<JointRecord>,
    headers: &'a [ContactHeader],
    islands: &'a Islands,
    depth: Shared<'a, u32>,
    used: Shared<'a, u32>,
    last: Shared<'a, u32>,
    order: Shared<'a, u32>,
    cache: Shared<'a, Vec<LocalBatch>>,
    volatile: Shared<'a, bool>,
    stamp: u32,
    lanes: u32,
}

impl IslandContext<'_> {
    fn schedule(&self, island: u32, scratch: &mut IslandScratch) {
        let slot = |body: u32| self.bodies.get(body).local as usize;
        let dynamic = |body: u32| self.bodies.get(body).island == island;
        let depth = |body: u32| unsafe { self.depth.get(slot(body)) };
        let used = |body: u32| unsafe { self.used.get(slot(body)) };
        let last = |body: u32| unsafe { self.last.get(slot(body)) };
        let order = |body: u32| unsafe { *self.order.at(slot(body)) as usize };
        let record = *self.islands.get(island);

        let mut members = 0;
        let mut body = record.bodies.head;
        while body != NONE {
            unsafe { *self.order.get(slot(body)) = members };
            *depth(body) = NONE;
            *used(body) = 0;
            *last(body) = NONE;
            members += 1;
            body = self.bodies.get(body).island_next;
        }

        let entries = &mut scratch.entries;
        entries.clear();
        let mut volatile = false;
        let mut contact = record.contacts.head;
        while contact != NONE {
            let c = self.contacts.get(contact);
            if c.flags & SKIP == 0 {
                let header = &self.headers[contact as usize];
                volatile |= header.kind == PairKind::Triangulated;
                let slots = if header.extra.stamp == self.stamp {
                    header.extra.count
                } else {
                    0
                };
                entries.extend((0..=slots).map(|slot| Entry {
                    depth: 0,
                    class: CONTACT_CLASS,
                    id: encode(contact, slot),
                    a: c.body_a(),
                    b: c.body_b(),
                }));
            }
            contact = c.island_next;
        }
        let mut joint = record.joints.head;
        while joint != NONE {
            let j = self.joints.get(joint);
            if j.params.kind() != JointKind::Filter {
                entries.push(Entry {
                    depth: 0,
                    class: class_of(j.params.kind(), j.params.layout()),
                    id: joint,
                    a: j.body_a,
                    b: j.body_b,
                });
            }
            joint = j.island_next;
        }

        let queue = &mut scratch.queue;
        queue.clear();
        for entry in entries.iter() {
            for (side, other) in [(entry.a, entry.b), (entry.b, entry.a)] {
                if dynamic(side) && !dynamic(other) && *depth(side) == NONE {
                    *depth(side) = 0;
                    queue.push_back(side);
                }
            }
        }
        if queue.is_empty() && record.bodies.head != NONE {
            *depth(record.bodies.head) = 0;
            queue.push_back(record.bodies.head);
        }
        let offsets = &mut scratch.offsets;
        offsets.clear();
        offsets.resize(members as usize + 2, 0);
        for entry in entries.iter() {
            if dynamic(entry.a) && dynamic(entry.b) {
                offsets[order(entry.a) + 2] += 1;
                offsets[order(entry.b) + 2] += 1;
            }
        }
        for index in 2..offsets.len() {
            offsets[index] += offsets[index - 1];
        }
        let neighbors = &mut scratch.neighbors;
        neighbors.resize(offsets[members as usize + 1] as usize, NONE);
        for entry in entries.iter() {
            if dynamic(entry.a) && dynamic(entry.b) {
                for (side, other) in [(entry.a, entry.b), (entry.b, entry.a)] {
                    let cursor = &mut offsets[order(side) + 1];
                    neighbors[*cursor as usize] = other;
                    *cursor += 1;
                }
            }
        }
        while let Some(current) = queue.pop_front() {
            let next_depth = *depth(current) + 1;
            let local = order(current);
            for &other in &neighbors[offsets[local] as usize..offsets[local + 1] as usize] {
                if *depth(other) == NONE {
                    *depth(other) = next_depth;
                    queue.push_back(other);
                }
            }
        }
        let depth_of = |body: u32| if dynamic(body) { *depth(body) } else { NONE };
        for entry in entries.iter_mut() {
            entry.depth = depth_of(entry.a).min(depth_of(entry.b));
        }
        entries.sort_unstable_by_key(|entry| {
            (
                entry.depth,
                entry.class != CONTACT_CLASS,
                entry.class,
                entry.id,
            )
        });

        let lanes = self.lanes;
        let (batches, joint_batches, open) = (
            &mut scratch.batches,
            &mut scratch.joint_batches,
            &mut scratch.open,
        );
        batches.clear();
        joint_batches.clear();
        open.clear();
        for entry in entries.iter() {
            if entry.class != CONTACT_CLASS {
                let last_of = |body: u32| if dynamic(body) { *last(body) } else { NONE };
                let start = match (last_of(entry.a), last_of(entry.b)) {
                    (NONE, NONE) => 0,
                    (NONE, x) | (x, NONE) => x as usize + 1,
                    (x, y) => x.max(y) as usize + 1,
                };
                let found = (start..joint_batches.len()).find(|&index| {
                    let batch = &joint_batches[index];
                    batch.class == entry.class && batch.count < lanes
                });
                let index = found.unwrap_or_else(|| {
                    joint_batches.push(LocalBatch::new(entry.class, 0));
                    joint_batches.len() - 1
                });
                let batch = &mut joint_batches[index];
                batch.ids[batch.count as usize] = entry.id;
                batch.count += 1;
                for body in [entry.a, entry.b] {
                    if dynamic(body) {
                        *last(body) = index as u32;
                    }
                }
                continue;
            }
            let used_by = |body: u32| if dynamic(body) { *used(body) } else { 0 };
            let taken = used_by(entry.a) | used_by(entry.b);
            let color = (!taken).trailing_zeros().min(OVERFLOW_COLOR);
            let key = (color as u64) << 32 | entry.class as u64;
            let found = if color == OVERFLOW_COLOR {
                None
            } else {
                open.iter()
                    .find(|&&(open_key, batch)| {
                        open_key == key && batches[batch as usize].count < lanes
                    })
                    .map(|&(_, batch)| batch as usize)
            };
            let index = found.unwrap_or_else(|| {
                batches.push(LocalBatch::new(entry.class, color));
                let index = batches.len() - 1;
                match open.iter_mut().find(|(open_key, _)| *open_key == key) {
                    Some(slot) => slot.1 = index as u32,
                    None => open.push((key, index as u32)),
                }
                index
            });
            let batch = &mut batches[index];
            batch.ids[batch.count as usize] = entry.id;
            batch.count += 1;
            if color != OVERFLOW_COLOR {
                for body in [entry.a, entry.b] {
                    if dynamic(body) {
                        *used(body) |= 1 << color;
                    }
                }
            }
        }
        batches.sort_by_key(|batch| batch.color);
        let cache = unsafe { self.cache.get(island as usize) };
        cache.clear();
        cache.extend_from_slice(joint_batches);
        cache.extend_from_slice(batches);
        *unsafe { self.volatile.get(island as usize) } = volatile;
    }
}

impl Scheduler {
    fn merge(&mut self, islands: &[u32], lanes: u32) {
        self.merged.clear();
        for &island in islands {
            let mut floor = 0;
            for batch in &self.cache[island as usize] {
                let mut ids = &batch.ids[..batch.count as usize];
                while !ids.is_empty() {
                    let open = self.merged[floor..]
                        .iter()
                        .position(|merged| merged.class == batch.class && merged.count < lanes)
                        .map(|offset| floor + offset);
                    let index = open.unwrap_or_else(|| {
                        self.merged.push(LocalBatch::new(batch.class, 0));
                        self.merged.len() - 1
                    });
                    let target = &mut self.merged[index];
                    let take = ((lanes - target.count) as usize).min(ids.len());
                    let start = target.count as usize;
                    target.ids[start..start + take].copy_from_slice(&ids[..take]);
                    target.count += take as u32;
                    ids = &ids[take..];
                    floor = index + 1;
                }
            }
        }
    }
}

impl RigidWorld {
    pub(crate) fn schedule_islands(&mut self, islands: &[u32], lanes: usize) {
        let scheduler = &mut self.solver.scheduler;
        let capacity = self.sets.awake().bodies.len();
        if scheduler.depth.len() < capacity {
            scheduler.depth.resize(capacity, NONE);
            scheduler.used.resize(capacity, 0);
            scheduler.last.resize(capacity, NONE);
            scheduler.order.resize(capacity, NONE);
        }
        let island_capacity = self.islands.records.len();
        if scheduler.cache.len() < island_capacity {
            scheduler.cache.resize_with(island_capacity, Vec::new);
            scheduler.cached.resize(island_capacity, false);
            scheduler.volatile.resize(island_capacity, false);
        }
        let workers = self.executor.workers();
        if scheduler.workers.len() != workers {
            scheduler.workers = PerWorker::new(workers, |_| IslandScratch::default());
        }
        scheduler.stale.clear();
        for &island in islands {
            let index = island as usize;
            let reuse = !self.islands.get(island).dirty
                && scheduler.cached[index]
                && !scheduler.volatile[index]
                && self.pre_solve.is_none();
            if !reuse {
                scheduler.stale.push(island);
                scheduler.cached[index] = true;
            }
        }
        if !scheduler.stale.is_empty() {
            let context = IslandContext {
                bodies: &self.bodies,
                contacts: &self.contacts,
                joints: &self.joints,
                headers: &self.headers,
                islands: &self.islands,
                depth: Shared::new(&mut scheduler.depth),
                used: Shared::new(&mut scheduler.used),
                last: Shared::new(&mut scheduler.last),
                order: Shared::new(&mut scheduler.order),
                cache: Shared::new(&mut scheduler.cache),
                volatile: Shared::new(&mut scheduler.volatile),
                stamp: self.narrowphase.stamp(),
                lanes: lanes as u32,
            };
            let (stale, workers) = (&scheduler.stale, &scheduler.workers);
            let task = |block: Block| {
                context.schedule(
                    stale[block.index as usize],
                    &mut workers.local(block.worker),
                );
            };
            self.executor
                .run(&[Stage::new("schedule islands", stale.len() as u32)], &task);
        }
        for &island in islands {
            self.islands.get_mut(island).dirty = false;
        }
    }

    pub(crate) fn schedule_groups(&mut self, islands: &[u32], plan: &mut Plan) {
        let target = GROUP_LANES * plan.lanes as u32;
        let mut start = 0;
        let mut size = 0;
        for (index, &island) in islands.iter().enumerate() {
            let record = self.islands.get(island);
            size += record.contacts.count + record.joints.count + record.bodies.count;
            if size >= target || index + 1 == islands.len() {
                self.schedule_group(&islands[start..=index], plan);
                start = index + 1;
                size = 0;
            }
        }
    }

    fn schedule_group(&mut self, islands: &[u32], plan: &mut Plan) {
        let bodies_start = plan.task_bodies.len() as u32;
        for &island in islands {
            let mut body = self.islands.get(island).bodies.head;
            while body != NONE {
                let record = self.bodies.get(body);
                plan.task_bodies.push(record.local + 1);
                body = record.island_next;
            }
        }
        let lanes = plan.lanes as u32;
        let scheduler = &mut self.solver.scheduler;
        let batches = if let [island] = islands {
            &scheduler.cache[*island as usize]
        } else {
            scheduler.merge(islands, lanes);
            &scheduler.merged
        };
        let sequence_start = plan.task_sequence.len() as u32;
        for batch in batches {
            let ids = &batch.ids[..batch.count as usize];
            let step = if batch.class == CONTACT_CLASS {
                TaskStep::Contacts(plan.push_contact_batch(ids))
            } else {
                let kind = JointKind::ALL[(batch.class >> 16) as usize];
                TaskStep::Joints(plan.push_joint_batch(kind, ids))
            };
            plan.task_sequence.push(step);
        }
        plan.tasks.push(TaskPlan {
            bodies: bodies_start..plan.task_bodies.len() as u32,
            sequence: sequence_start..plan.task_sequence.len() as u32,
        });
    }
}
