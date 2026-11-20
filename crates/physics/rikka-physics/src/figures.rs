use std::sync::Arc;

use rikka_model_core::Instance;

use crate::scene_source::{FigureMotion, SceneSource};

pub(crate) struct Figures {
    instances: Vec<Instance>,
    resting: Vec<bool>,
}

impl Figures {
    pub(crate) fn new(source: &dyn SceneSource) -> Figures {
        let instances: Vec<Instance> = source
            .layout()
            .figures
            .iter()
            .map(|figure| Instance::new(Arc::clone(&figure.model)))
            .collect();
        Figures {
            resting: vec![false; instances.len()],
            instances,
        }
    }

    pub(crate) fn instances(&self) -> &[Instance] {
        &self.instances
    }

    pub(crate) fn wake_figures(&mut self) {
        self.resting.fill(false);
    }

    pub(crate) fn pose(&mut self, source: &dyn SceneSource, time: f32) {
        for (index, instance) in self.instances.iter_mut().enumerate() {
            if self.resting[index] {
                continue;
            }
            self.resting[index] = source.pose(index, time, instance) == FigureMotion::Resting;
            instance.refresh();
        }
    }
}
