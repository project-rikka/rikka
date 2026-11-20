use std::sync::Arc;

use super::grip_points;
use crate::holding::ClothSnapshot;
use crate::render::Render;
use crate::{Projection, Stage, StageError};

impl Stage {
    pub fn cloth_snapshot(&mut self) -> Option<Arc<ClothSnapshot>> {
        if !self.outputs.graspable {
            self.outputs.cloth_snapshot = None;
            return None;
        }
        self.wants_snapshot = true;
        self.roused = true;
        self.outputs.cloth_snapshot.clone()
    }

    pub fn graspable(&self) -> bool {
        self.outputs.graspable
    }
}

impl Render {
    pub fn snapshot_cloth(&mut self) {
        match self.simulation.cloth_snapshot(&self.actors) {
            Ok(Some(cloth_snapshot)) => {
                self.outputs.cloth_snapshot = Some(Arc::new(cloth_snapshot));
            }
            Ok(None) => {}
            Err(error) => self.fail(StageError::Cloth(error)),
        }
    }

    pub fn project_grips(&self, projection: &mut Projection, time: f32) {
        let Some(holder) = self
            .actors
            .iter()
            .position(|actor| actor.id == projection.figure)
        else {
            return;
        };
        projection.grips = grip_points(time, &self.simulation.performers(&self.actors), holder);
    }
}
