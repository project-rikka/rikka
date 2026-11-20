use animate_motion::CameraMotion;

use crate::sheet::{Group, GroupKind, Lane, Sheet};

impl Sheet {
    #[must_use]
    pub fn camera(motion: &CameraMotion) -> Self {
        Self::new(vec![Group::keyed(
            GroupKind::Camera,
            Some(Lane::Camera),
            motion.keys().iter().map(|key| key.frame).collect(),
        )])
    }
}
