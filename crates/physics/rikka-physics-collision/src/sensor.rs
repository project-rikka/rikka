use rikka_physics_core::{Executor, ExecutorExt};
use rikka_physics_geometry::{ProxyBuffer, ShapeGeometry};

use crate::{BodyFrame, Broadphase, Collider};

const NONE: u32 = u32::MAX;
const BLOCK: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShapeKey {
    pub collider: u32,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SensorEvent {
    pub sensor: ShapeKey,
    pub visitor: ShapeKey,
}

#[derive(Debug, Default)]
struct SensorState {
    key: ShapeKey,
    previous: Vec<ShapeKey>,
    current: Vec<ShapeKey>,
    changed: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct SensorInput<'a> {
    pub broadphase: &'a Broadphase,
    pub colliders: &'a [Collider],
    pub frames: &'a [BodyFrame],
    pub generations: &'a [u32],
}

#[derive(Debug, Default)]
pub struct Sensors {
    sensors: Vec<SensorState>,
    index_of: Vec<u32>,
    begin: Vec<SensorEvent>,
    end: Vec<SensorEvent>,
    removed: Vec<SensorEvent>,
}

impl Sensors {
    pub fn len(&self) -> usize {
        self.sensors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sensors.is_empty()
    }

    pub fn begin_events(&self) -> &[SensorEvent] {
        &self.begin
    }

    pub fn end_events(&self) -> &[SensorEvent] {
        &self.end
    }

    pub fn overlaps(&self, collider: u32) -> &[ShapeKey] {
        match self.index_of.get(collider as usize) {
            Some(&index) if index != NONE => &self.sensors[index as usize].previous,
            _ => &[],
        }
    }

    pub fn add(&mut self, key: ShapeKey) {
        let collider = key.collider as usize;
        if self.index_of.len() <= collider {
            self.index_of.resize(collider + 1, NONE);
        }
        self.index_of[collider] = self.sensors.len() as u32;
        self.sensors.push(SensorState {
            key,
            ..Default::default()
        });
    }

    pub fn remove(&mut self, collider: u32) {
        let Some(&index) = self.index_of.get(collider as usize) else {
            return;
        };
        if index == NONE {
            return;
        }
        self.index_of[collider as usize] = NONE;
        let state = self.sensors.swap_remove(index as usize);
        self.removed
            .extend(state.previous.iter().map(|&visitor| SensorEvent {
                sensor: state.key,
                visitor,
            }));
        if let Some(moved) = self.sensors.get(index as usize) {
            self.index_of[moved.key.collider as usize] = index;
        }
    }

    pub fn update(&mut self, executor: &dyn Executor, input: &SensorInput<'_>) {
        self.begin.clear();
        self.end.clear();
        self.end.append(&mut self.removed);
        if self.sensors.is_empty() {
            return;
        }
        executor.for_each_chunk("sensors", &mut self.sensors, BLOCK, |_, states| {
            let mut buffers = [ProxyBuffer::default(); 2];
            for state in states {
                overlap(state, input, &mut buffers);
                state.changed = state.current != state.previous;
            }
        });
        for state in &mut self.sensors {
            if state.changed {
                diff(state, &mut self.begin, &mut self.end);
            }
            std::mem::swap(&mut state.previous, &mut state.current);
        }
    }
}

fn overlap(state: &mut SensorState, input: &SensorInput<'_>, buffers: &mut [ProxyBuffer; 2]) {
    state.current.clear();
    let sensor = &input.colliders[state.key.collider as usize];
    let frame = input.frames[sensor.owner as usize].transform;
    let bounds = sensor.shape.aabb(frame);
    let [sensor_buffer, visitor_buffer] = buffers;
    let sensor_proxy = sensor.shape.proxy(sensor_buffer);
    input.broadphase.query(&bounds, |other| {
        let visitor = &input.colliders[other as usize];
        if visitor.sensor
            || !visitor.sensor_events
            || visitor.owner == sensor.owner
            || !sensor.filter.should_collide(&visitor.filter)
            || !visitor
                .shape
                .aabb(input.frames[visitor.owner as usize].transform)
                .overlaps(&bounds)
        {
            return true;
        }
        let visitor_frame = input.frames[visitor.owner as usize].transform;
        let touching = match &sensor_proxy {
            Some(proxy) => visitor
                .shape
                .overlaps(frame.inverse_mul(&visitor_frame), proxy),
            None => visitor.shape.proxy(visitor_buffer).is_some_and(|proxy| {
                sensor
                    .shape
                    .overlaps(visitor_frame.inverse_mul(&frame), &proxy)
            }),
        };
        if touching {
            state.current.push(ShapeKey {
                collider: other,
                generation: input.generations[other as usize],
            });
        }
        true
    });
    state.current.sort_unstable();
    state.current.dedup();
}

fn diff(state: &SensorState, begin: &mut Vec<SensorEvent>, end: &mut Vec<SensorEvent>) {
    let (old, new) = (&state.previous, &state.current);
    let sensor = state.key;
    let (mut i, mut j) = (0, 0);
    while i < old.len() || j < new.len() {
        match (old.get(i), new.get(j)) {
            (Some(a), Some(b)) if a == b => {
                i += 1;
                j += 1;
            }
            (Some(a), Some(b)) if a > b => {
                begin.push(SensorEvent {
                    sensor,
                    visitor: *b,
                });
                j += 1;
            }
            (Some(a), _) => {
                end.push(SensorEvent {
                    sensor,
                    visitor: *a,
                });
                i += 1;
            }
            (None, Some(b)) => {
                begin.push(SensorEvent {
                    sensor,
                    visitor: *b,
                });
                j += 1;
            }
            (None, None) => break,
        }
    }
}
