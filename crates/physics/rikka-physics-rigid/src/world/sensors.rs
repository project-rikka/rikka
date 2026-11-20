use rikka_physics_collision::{SensorInput, ShapeKey};

use super::RigidWorld;
use crate::{Result, SensorBeginEvent, SensorEndEvent, ShapeId};

fn shape_id(key: ShapeKey) -> ShapeId {
    ShapeId {
        index: key.collider,
        generation: key.generation,
    }
}

impl RigidWorld {
    pub(crate) fn update_sensors(&mut self) {
        let executor = self.executor.clone();
        let input = SensorInput {
            broadphase: &self.broadphase,
            colliders: &self.colliders,
            frames: &self.frames,
            generations: self.shapes.generations(),
        };
        self.sensors.update(&*executor, &input);
        self.events
            .sensor_begin
            .extend(
                self.sensors
                    .begin_events()
                    .iter()
                    .map(|event| SensorBeginEvent {
                        sensor: shape_id(event.sensor),
                        visitor: shape_id(event.visitor),
                    }),
            );
        self.events
            .sensor_end
            .extend(
                self.sensors
                    .end_events()
                    .iter()
                    .map(|event| SensorEndEvent {
                        sensor: shape_id(event.sensor),
                        visitor: shape_id(event.visitor),
                    }),
            );
    }

    pub fn sensor_begin_events(&self) -> &[SensorBeginEvent] {
        &self.events.sensor_begin
    }

    pub fn sensor_end_events(&self) -> &[SensorEndEvent] {
        &self.events.sensor_end
    }

    pub fn sensor_overlaps(&self, sensor: ShapeId) -> Result<impl Iterator<Item = ShapeId> + '_> {
        let index = self.shape_index(sensor)?;
        Ok(self
            .sensors
            .overlaps(index)
            .iter()
            .copied()
            .filter(|key| self.shapes.is_alive(key.collider, key.generation))
            .map(shape_id))
    }
}
